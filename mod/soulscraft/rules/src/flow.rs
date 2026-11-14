//! The path map of an altar's event zone (06-altar.md, «Путь к алтарю»): one flow field from the
//! altar over the 3 × 3 chunks, built when the zone's blocks change (at most once a second), which
//! every wave mob follows instead of looking for a path of its own.
//!
//! The zone is a box of cells, each [`Cell`]: open air, a low block (a slab, a carpet: walked on
//! and over), a wall a brute breaks in some hits, or a hard block nothing breaks. A mob stands in
//! a cell when it and the one above are walkable and something holds it up. It steps to the four
//! cells beside it, one up (with room for its head) or down as far as three.
//!
//! Two fields come out of one build: `open`, steps to the altar through walkable cells only (no
//! path: the mob is walled out, and after a while it curses the altar), and `breach`, the cost to
//! the altar when walls may be broken (the brute, the only breaker: it walks it and hits the
//! walls on its way). Both are distances from the seed cells round the altar, so a mob goes to a
//! neighbour with a smaller one.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

pub type Pos = [i32; 3];

/// No way from this cell.
pub const NONE: u32 = u32::MAX;
/// The cost of one hit on a wall, in steps: a brute spends a few seconds on a block, about what
/// it takes to walk a few blocks round it.
pub const HIT_COST: u32 = 4;
/// The farthest a mob drops.
pub const MAX_DROP: i32 = 3;
const DIRS: [[i32; 2]; 4] = [[0, 1], [1, 0], [0, -1], [-1, 0]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// No collision.
    Open,
    /// A low collision box (a slab, a carpet): walked over and stood on.
    Low,
    /// A full block a brute breaks in this many hits (1 or more).
    Wall(u8),
    /// A block nothing breaks: an altar block, a machine, bedrock, or past the zone's edge.
    Hard,
}

impl Cell {
    pub fn passable(self) -> bool {
        matches!(self, Cell::Open | Cell::Low)
    }
    fn supports(self) -> bool {
        !matches!(self, Cell::Open)
    }
    fn breach_passable(self) -> bool {
        matches!(self, Cell::Open | Cell::Low | Cell::Wall(_))
    }
    fn hits(self) -> u32 {
        match self {
            Cell::Wall(h) => u32::from(h),
            _ => 0,
        }
    }
}

/// The zone's cells: `min` is the lowest north-west corner, `dims` the size in x, y and z.
#[derive(Clone, Debug)]
pub struct Grid {
    pub min: Pos,
    pub dims: [i32; 3],
    cells: Vec<Cell>,
}

impl Grid {
    pub fn new(min: Pos, dims: [i32; 3], fill: Cell) -> Self {
        let n = (dims[0].max(0) * dims[1].max(0) * dims[2].max(0)) as usize;
        Self { min, dims, cells: vec![fill; n] }
    }

    pub fn index(&self, p: Pos) -> Option<usize> {
        let l = [p[0] - self.min[0], p[1] - self.min[1], p[2] - self.min[2]];
        if (0..3).any(|i| l[i] < 0 || l[i] >= self.dims[i]) {
            return None;
        }
        Some(((l[1] * self.dims[2] + l[2]) * self.dims[0] + l[0]) as usize)
    }

    pub fn contains(&self, p: Pos) -> bool {
        self.index(p).is_some()
    }

    /// The cell; outside the box everything is hard (the zone is fenced).
    pub fn get(&self, p: Pos) -> Cell {
        self.index(p).map_or(Cell::Hard, |i| self.cells[i])
    }

    pub fn set(&mut self, p: Pos, c: Cell) {
        if let Some(i) = self.index(p) {
            self.cells[i] = c;
        }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    fn up(p: Pos, k: i32) -> Pos {
        [p[0], p[1] + k, p[2]]
    }

    /// A mob can stand in `p`: it and the cell above walkable, held up by the cell below or by a
    /// low block in `p` itself.
    pub fn standable(&self, p: Pos) -> bool {
        let here = self.get(p);
        here.passable() && self.get(Self::up(p, 1)).passable() && (self.get(Self::up(p, -1)).supports() || here == Cell::Low)
    }

    /// As [`Grid::standable`] when walls are broken through.
    fn breach_standable(&self, p: Pos) -> bool {
        let here = self.get(p);
        here.breach_passable()
            && self.get(Self::up(p, 1)).breach_passable()
            && (self.get(Self::up(p, -1)).supports() || here == Cell::Low)
    }

    /// `height` free cells over `p` (a tall mob's room) and `p` standable.
    pub fn headroom(&self, p: Pos, height: i32) -> bool {
        self.standable(p) && (0..height).all(|k| self.get(Self::up(p, k)).passable())
    }

    /// A move from `a` to `b` (beside it, `dy` from −3 to +1) through walkable cells.
    fn can_move(&self, a: Pos, b: Pos) -> bool {
        let dy = b[1] - a[1];
        if !(-MAX_DROP..=1).contains(&dy) || !self.standable(b) {
            return false;
        }
        if dy == 1 {
            return self.get(Self::up(a, 2)).passable();
        }
        // Falling down the column at b: the cells from just over b's head to a's head are free.
        (b[1] + 2..=a[1] + 1).all(|y| self.get([b[0], y, b[2]]).passable())
    }

    /// The same move when walls may be broken; its cost in steps, or None.
    fn breach_cost(&self, a: Pos, b: Pos) -> Option<u32> {
        let dy = b[1] - a[1];
        if !(-MAX_DROP..=1).contains(&dy) || !self.breach_standable(b) {
            return None;
        }
        let mut hits = self.get(b).hits() + self.get(Self::up(b, 1)).hits();
        if dy == 1 {
            let head = self.get(Self::up(a, 2));
            if !head.breach_passable() {
                return None;
            }
            hits += head.hits();
        } else {
            for y in b[1] + 2..=a[1] + 1 {
                let c = self.get([b[0], y, b[2]]);
                if !c.breach_passable() {
                    return None;
                }
                hits += c.hits();
            }
        }
        Some(1 + hits * HIT_COST)
    }

    /// The cells a mob in `a` may come from to reach `b`: beside it, one down or up to three up.
    fn sources(b: Pos) -> impl Iterator<Item = Pos> {
        DIRS.into_iter()
            .flat_map(move |[dx, dz]| (-1..=MAX_DROP).map(move |up| [b[0] - dx, b[1] + up, b[2] - dz]))
    }

    /// The cells a mob in `a` may go to.
    fn targets(a: Pos) -> impl Iterator<Item = Pos> {
        DIRS.into_iter().flat_map(move |[dx, dz]| (-MAX_DROP..=1).map(move |dy| [a[0] + dx, a[1] + dy, a[2] + dz]))
    }

    /// The walls between `a` and `b` that a breaker hits, nearest the feet first.
    pub fn walls_between(&self, a: Pos, b: Pos) -> Vec<Pos> {
        let mut out = Vec::new();
        let mut take = |p: Pos| {
            if matches!(self.get(p), Cell::Wall(_)) {
                out.push(p);
            }
        };
        if b[1] > a[1] {
            take(Self::up(a, 2));
        }
        take(b);
        take(Self::up(b, 1));
        for y in b[1] + 2..=a[1] + 1 {
            take([b[0], y, b[2]]);
        }
        out
    }
}

/// The two distance fields of one build.
#[derive(Clone, Debug)]
pub struct Field {
    pub grid: Grid,
    open: Vec<u32>,
    breach: Vec<u32>,
    /// The standable seeds round the altar.
    pub seeds: Vec<Pos>,
}

impl Field {
    /// Builds both fields from `seeds` (cells where a mob is at the altar).
    pub fn build(grid: Grid, seeds: &[Pos]) -> Self {
        let n = grid.len();
        let mut open = vec![NONE; n];
        let mut queue = VecDeque::new();
        let seeds: Vec<Pos> = seeds.iter().copied().filter(|s| grid.standable(*s)).collect();
        for s in &seeds {
            if let Some(i) = grid.index(*s) {
                open[i] = 0;
                queue.push_back(*s);
            }
        }
        while let Some(b) = queue.pop_front() {
            let d = open[grid.index(b).unwrap()];
            for a in Grid::sources(b) {
                let Some(i) = grid.index(a) else { continue };
                if open[i] == NONE && grid.standable(a) && grid.can_move(a, b) {
                    open[i] = d + 1;
                    queue.push_back(a);
                }
            }
        }
        let mut breach = vec![NONE; n];
        let mut heap = BinaryHeap::new();
        for s in &seeds {
            if let Some(i) = grid.index(*s) {
                breach[i] = 0;
                heap.push(Reverse((0u32, *s)));
            }
        }
        while let Some(Reverse((d, b))) = heap.pop() {
            if d > breach[grid.index(b).unwrap()] {
                continue;
            }
            for a in Grid::sources(b) {
                let Some(i) = grid.index(a) else { continue };
                if !grid.breach_standable(a) {
                    continue;
                }
                let Some(cost) = grid.breach_cost(a, b) else { continue };
                let nd = d.saturating_add(cost);
                if nd < breach[i] {
                    breach[i] = nd;
                    heap.push(Reverse((nd, a)));
                }
            }
        }
        Self { grid, open, breach, seeds }
    }

    pub fn open_at(&self, p: Pos) -> Option<u32> {
        self.grid.index(p).map(|i| self.open[i]).filter(|d| *d != NONE)
    }

    pub fn breach_at(&self, p: Pos) -> Option<u32> {
        self.grid.index(p).map(|i| self.breach[i]).filter(|d| *d != NONE)
    }

    /// The cell a mob at block position `p` stands in: `p`, or one above or below it (a mob on a
    /// slab, a mob in a jump).
    pub fn locate(&self, p: Pos) -> Option<Pos> {
        [p, [p[0], p[1] - 1, p[2]], [p[0], p[1] + 1, p[2]]]
            .into_iter()
            .find(|c| self.grid.standable(*c) || self.grid.breach_standable(*c) && self.breach_at(*c).is_some())
    }

    /// The next cell toward the altar through walkable cells.
    pub fn next_open(&self, p: Pos) -> Option<Pos> {
        let d = self.open_at(p)?;
        if d == 0 {
            return None;
        }
        Grid::targets(p)
            .filter(|b| self.open_at(*b).is_some_and(|db| db < d) && self.grid.can_move(p, *b))
            .min_by_key(|b| self.open_at(*b).unwrap_or(NONE))
    }

    /// Up to `steps` cells along the walkable way from `p`, the last one the farthest reached.
    pub fn ahead(&self, p: Pos, steps: usize) -> Vec<Pos> {
        let mut out = Vec::new();
        let mut at = p;
        for _ in 0..steps {
            let Some(n) = self.next_open(at) else { break };
            out.push(n);
            at = n;
        }
        out
    }

    /// The breaker's next cell toward the altar and the walls on the way there.
    pub fn next_breach(&self, p: Pos) -> Option<(Pos, Vec<Pos>)> {
        let d = self.breach_at(p)?;
        if d == 0 {
            return None;
        }
        Grid::targets(p)
            .filter_map(|b| {
                let db = self.breach_at(b)?;
                let cost = self.grid.breach_cost(p, b)?;
                (db + cost == d).then_some((db, b))
            })
            .min_by_key(|(db, _)| *db)
            .map(|(_, b)| (b, self.grid.walls_between(p, b)))
    }

    /// Standable cells the altar can be reached from, between `near` and `far` blocks from
    /// `centre` on the ground: where a wave's mobs may appear.
    pub fn spawn_cells(&self, centre: Pos, near: f64, far: f64) -> Vec<Pos> {
        let mut out = Vec::new();
        let g = &self.grid;
        for y in g.min[1]..g.min[1] + g.dims[1] {
            for z in g.min[2]..g.min[2] + g.dims[2] {
                for x in g.min[0]..g.min[0] + g.dims[0] {
                    let d = (f64::from(x - centre[0]).powi(2) + f64::from(z - centre[2]).powi(2)).sqrt();
                    if d < near || d > far {
                        continue;
                    }
                    let p = [x, y, z];
                    if self.open_at(p).is_some() {
                        out.push(p);
                    }
                }
            }
        }
        out
    }
}

/// `count` spawn points spread round the altar from `cells`: the cells are sorted by their
/// angle round `centre`, and the points taken at even steps from an offset `seed` picks, so a
/// wave comes from several sides and two events differ.
pub fn pick_spread(cells: &[Pos], centre: Pos, count: usize, seed: u64) -> Vec<Pos> {
    if cells.is_empty() || count == 0 {
        return Vec::new();
    }
    let mut sorted = cells.to_vec();
    let angle = |p: &Pos| f64::from(p[2] - centre[2]).atan2(f64::from(p[0] - centre[0]));
    sorted.sort_by(|a, b| angle(a).total_cmp(&angle(b)).then(a.cmp(b)));
    let n = sorted.len();
    let offset = (seed as usize) % n;
    (0..count).map(|i| sorted[(offset + i * n / count.min(n).max(1)) % n]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat floor at y 0 (stone) with air above, x and z from −10 to 10, y −1 to 4.
    fn flat() -> Grid {
        let mut g = Grid::new([-10, -1, -10], [21, 6, 21], Cell::Open);
        for x in -10..=10 {
            for z in -10..=10 {
                g.set([x, -1, z], Cell::Hard);
            }
        }
        g
    }

    fn wall_ring(g: &mut Grid, r: i32, cell: Cell, gap: Option<Pos>) {
        for x in -r..=r {
            for z in -r..=r {
                if x.abs().max(z.abs()) == r {
                    for y in 0..3 {
                        if gap != Some([x, y, z]) && gap.map(|p| [p[0], p[1] + 1, p[2]]) != Some([x, y, z]) {
                            g.set([x, y, z], cell);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn open_ground_leads_straight_to_the_altar() {
        let f = Field::build(flat(), &[[0, 0, 0]]);
        assert_eq!(f.open_at([0, 0, 0]), Some(0));
        assert_eq!(f.open_at([5, 0, 0]), Some(5));
        assert_eq!(f.open_at([3, 0, 4]), Some(7));
        assert_eq!(f.next_open([5, 0, 0]), Some([4, 0, 0]));
        assert_eq!(f.ahead([5, 0, 0], 10).last(), Some(&[0, 0, 0]));
        assert_eq!(f.next_open([0, 0, 0]), None);
        assert_eq!(f.locate([5, 1, 0]), Some([5, 0, 0]));
        // Mid-air is no cell of the field.
        assert_eq!(f.open_at([5, 2, 0]), None);
    }

    #[test]
    fn a_closed_wall_leaves_no_path_but_a_breach() {
        let mut g = flat();
        wall_ring(&mut g, 3, Cell::Wall(2), None);
        let f = Field::build(g, &[[0, 0, 0]]);
        assert_eq!(f.open_at([6, 0, 0]), None);
        let b = f.breach_at([6, 0, 0]).unwrap();
        // 6 steps and two wall cells of 2 hits each (feet and head), 3 tall walls leave the third.
        assert_eq!(b, 6 + 4 * HIT_COST);
        let mut at = [6, 0, 0];
        let mut walls = Vec::new();
        for _ in 0..10 {
            let Some((n, w)) = f.next_breach(at) else { break };
            walls.extend(w);
            at = n;
        }
        assert_eq!(at, [0, 0, 0]);
        assert_eq!(walls, vec![[3, 0, 0], [3, 1, 0]]);
    }

    #[test]
    fn a_gap_in_the_wall_is_the_way_even_when_long() {
        let mut g = flat();
        wall_ring(&mut g, 3, Cell::Wall(1), Some([0, 0, -3]));
        let f = Field::build(g, &[[0, 0, 0]]);
        // From the east: round to the north gap, longer than straight.
        let d = f.open_at([6, 0, 0]).unwrap();
        assert!(d > 6, "{d}");
        assert!(f.ahead([6, 0, 0], 40).contains(&[0, 0, -3]));
    }

    #[test]
    fn hard_walls_are_never_broken() {
        let mut g = flat();
        wall_ring(&mut g, 3, Cell::Hard, None);
        let f = Field::build(g, &[[0, 0, 0]]);
        assert_eq!(f.open_at([6, 0, 0]), None);
        assert_eq!(f.breach_at([6, 0, 0]), None);
        assert_eq!(f.next_breach([6, 0, 0]), None);
    }

    #[test]
    fn steps_up_one_and_drops_three() {
        let mut g = flat();
        // A 1-high step at x = 3..: walkable on top, at y 1.
        for x in 3..=10 {
            for z in -10..=10 {
                g.set([x, 0, z], Cell::Hard);
            }
        }
        let f = Field::build(g.clone(), &[[0, 0, 0]]);
        assert_eq!(f.open_at([5, 1, 0]), Some(5));
        // A 2-high cliff: a mob on top drops down to the altar below...
        for x in 3..=10 {
            for z in -10..=10 {
                g.set([x, 1, z], Cell::Hard);
            }
        }
        let f = Field::build(g.clone(), &[[0, 0, 0]]);
        assert_eq!(f.open_at([5, 2, 0]), Some(5));
        // ...but cannot climb up to an altar on top of it.
        let f = Field::build(g, &[[6, 2, 0]]);
        assert_eq!(f.open_at([0, 0, 0]), None);
        assert!(f.open_at([4, 2, 0]).is_some());
    }

    #[test]
    fn slabs_are_walked_over() {
        let mut g = flat();
        for x in -2..=2 {
            for z in -2..=2 {
                g.set([x, 0, z], Cell::Low);
            }
        }
        let f = Field::build(g, &[[0, 0, 0]]);
        assert_eq!(f.open_at([2, 0, 0]), Some(2));
        assert_eq!(f.open_at([4, 0, 0]), Some(4));
    }

    #[test]
    fn spawn_cells_lie_in_the_band_and_spread() {
        let f = Field::build(flat(), &[[0, 0, 0]]);
        let cells = f.spawn_cells([0, 0, 0], 6.0, 8.0);
        assert!(!cells.is_empty());
        assert!(cells.iter().all(|p| {
            let d = f64::from(p[0] * p[0] + p[2] * p[2]).sqrt();
            (6.0..=8.0).contains(&d) && p[1] == 0
        }));
        let picked = pick_spread(&cells, [0, 0, 0], 4, 7);
        assert_eq!(picked.len(), 4);
        // Four points spread: the quadrants differ.
        let mut q: Vec<(bool, bool)> = picked.iter().map(|p| (p[0] >= 0, p[2] >= 0)).collect();
        q.sort();
        q.dedup();
        assert!(q.len() >= 3, "{picked:?}");
        assert!(pick_spread(&[], [0, 0, 0], 3, 1).is_empty());
        // More wanted than there are cells: cells repeat.
        assert_eq!(pick_spread(&[[1, 0, 1]], [0, 0, 0], 3, 1).len(), 3);
    }

    #[test]
    fn headroom_for_tall_mobs() {
        let mut g = flat();
        g.set([4, 3, 0], Cell::Hard);
        assert!(g.headroom([4, 0, 0], 3));
        assert!(!g.headroom([4, 0, 0], 4));
        assert!(!g.contains([40, 0, 0]));
        assert_eq!(g.get([40, 0, 0]), Cell::Hard);
        assert!(!g.is_empty());
    }
}
