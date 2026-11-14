use super::*;
use crate::book_page::{self, Seg, Tone};
use serde_json::json;

fn book() -> BookFile {
    BookFile::embedded()
}

fn entry(c: &Catalog, id: &str) -> Entry {
    c.get(id).unwrap().0.clone()
}

fn trophies() -> Value {
    serde_json::from_str(crate::event::TROPHIES).unwrap()
}

#[test]
fn the_file_reads_with_its_stages_and_credit() {
    let b = book();
    assert_eq!(b.item, ITEM);
    assert_eq!(b.sections, ["Враги", "Алтарь", "Ивенты"]);
    let names: Vec<&str> = b.stages.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Неизвестен", "Встречен", "Убит", "Изучен", "Знаток"]);
    assert_eq!(b.credit, Credit { radius_blocks: 32.0, window_seconds: 10.0, spawner_weight: 0.25 });
    assert_eq!(b.window_ticks(), 200);
    assert!(b.altar_entry("defense").unwrap().text.contains("лабиринт"));
    assert!(!b.altar_entry("altar").unwrap().text.is_empty());
    assert!(b.altar_entry("nothing").is_none());
}

#[test]
fn bad_files_are_refused() {
    assert!(BookFile::parse("{").unwrap_err().contains("book.json"));
    let other = EMBEDDED.replace("\"soulscraft-book\"", "\"x\"");
    assert!(BookFile::parse(&other).unwrap_err().contains("format"));
    let down = EMBEDDED.replace("\"kills\": 15, \"elite_kills\": 8", "\"kills\": 2, \"elite_kills\": 8");
    assert!(BookFile::parse(&down).unwrap_err().contains("fewer"));
    let none = EMBEDDED.replace("\"kills\": 5, \"elite_kills\": 3", "\"kills\": null, \"elite_kills\": 3");
    assert!(BookFile::parse(&none).unwrap_err().contains("without kills"));
    let weight = EMBEDDED.replace("\"spawner_weight\": 0.25", "\"spawner_weight\": 0");
    assert!(BookFile::parse(&weight).unwrap_err().contains("spawner_weight"));
    let four = EMBEDDED.replace("{\"id\": \"expert\"", "{\"id\": \"expert\", \"n\": 7, \"x\"");
    assert!(BookFile::parse(&four).is_err());
}

#[test]
fn stages_follow_kills_and_elite_mobs_open_sooner() {
    let b = book();
    assert_eq!(b.stage(false, 0.0, false), UNKNOWN);
    assert_eq!(b.stage(true, 0.0, false), MET);
    // A spawner's quarter kill: met, not killed.
    assert_eq!(b.stage(false, 0.25, false), MET);
    for (kills, normal, elite) in [(1.0, KILLED, KILLED), (2.0, KILLED, KILLED), (3.0, KILLED, STUDIED), (4.99, KILLED, STUDIED), (5.0, STUDIED, STUDIED), (8.0, STUDIED, EXPERT), (14.0, STUDIED, EXPERT), (15.0, EXPERT, EXPERT)] {
        assert_eq!(b.stage(true, kills, false), normal, "{kills} kills");
        assert_eq!(b.stage(true, kills, true), elite, "{kills} elite kills");
    }
    assert_eq!((b.need(KILLED, true), b.need(STUDIED, true), b.need(EXPERT, true)), (1.0, 3.0, 8.0));
    assert_eq!((b.need(KILLED, false), b.need(STUDIED, false), b.need(EXPERT, false)), (1.0, 5.0, 15.0));
    assert_eq!(b.stage_name(9), "Знаток");
    // Four quarters make a kill.
    let mut p = Progress::default();
    let c = Catalog::embedded();
    let r = entry(&c, "revenant");
    for _ in 0..3 {
        p.kill(&b, &r, weight(&b, true));
    }
    assert_eq!(p.stage(&b, &r), MET);
    assert_eq!(p.kill(&b, &r, weight(&b, true)).unwrap().stage, "Убит");
    assert_eq!(p.mob("revenant").kills, 1.0);
}

#[test]
fn the_catalog_has_every_mob_of_the_data_and_its_types() {
    let c = Catalog::embedded();
    let data = crate::data::MobsFile::embedded();
    assert!(c.entries.len() >= data.mobs.len());
    for m in &data.mobs {
        let e = c.by_key(&m.key).unwrap();
        assert_eq!((e.id.as_str(), e.name.as_str(), e.elite, e.vanilla), (m.id.as_str(), m.name.as_str(), m.elite, false));
    }
    assert!(c.by_key("soulscraft:brute").unwrap().elite);
    assert!(c.by_key("minecraft:pig").is_none());
    assert_eq!(c.event_name("test_zombies"), "Тестовый ивент первого среза");
    assert_eq!(c.event_name("nope"), "nope");
    // The changed vanilla mobs of the data (07-vanilla-mobs.md): one entry a row, every type of it.
    let root: Value = serde_json::from_str(crate::data::EMBEDDED).unwrap();
    if let Some(groups) = root["vanilla"]["groups"].as_array() {
        for g in groups {
            for t in g["types"].as_array().unwrap() {
                let e = c.by_key(t.as_str().unwrap()).unwrap();
                assert_eq!((e.id.as_str(), e.vanilla), (g["id"].as_str().unwrap(), true));
            }
        }
        assert_eq!(c.entries.len(), data.mobs.len() + groups.len());
    }
}

#[test]
fn changed_vanilla_mobs_get_entries_whatever_the_list_looks_like() {
    let base = json!({"mobs": [{"id": "revenant", "key": "soulscraft:revenant", "name": "Латник-мертвец"}]});
    let shapes = [
        json!([{"id": "zombie", "name": "Зомби", "keys": ["minecraft:zombie", "minecraft:husk"]}, {"id": "skeleton", "name": "Скелет"}]),
        json!({"mobs": [{"id": "zombie", "name": "Зомби", "keys": ["minecraft:zombie", "minecraft:husk"]}, {"id": "skeleton", "name": "Скелет"}]}),
        json!({"zombie": {"name": "Зомби", "keys": ["minecraft:zombie", "minecraft:husk"]}, "skeleton": {"name": "Скелет", "key": "minecraft:skeleton"}, "note": "text"}),
        json!({"source": "x", "groups": [{"id": "zombie", "name": "Зомби", "types": ["minecraft:zombie", "minecraft:husk"]}, {"id": "skeleton", "name": "Скелет", "types": ["minecraft:skeleton"]}]}),
    ];
    for v in shapes {
        let mut root = base.clone();
        root["vanilla"] = v;
        let c = Catalog::from_value(&root);
        assert_eq!(c.entries.len(), 3, "{root}");
        let z = c.by_key("minecraft:husk").unwrap();
        assert_eq!((z.id.as_str(), z.vanilla), ("zombie", true));
        assert_eq!(c.by_key("minecraft:skeleton").unwrap().name, "Скелет");
        assert!(!c.by_key("soulscraft:revenant").unwrap().vanilla);
    }
    // A vanilla type listed with the mobs, an entry without an id, a repeated id.
    let root = json!({"mobs": [{"id": "drowned", "key": "minecraft:drowned"}, {"name": "x"}, {"id": "drowned", "key": "minecraft:zombie"}]});
    let c = Catalog::from_value(&root);
    assert_eq!(c.entries.len(), 1);
    assert!(c.by_key("minecraft:drowned").unwrap().vanilla);
    assert_eq!(c.by_key("minecraft:drowned").unwrap().name, "drowned");
    assert!(Catalog::parse("[").is_err());
}

#[test]
fn meeting_and_killing_give_one_line_per_new_stage() {
    let b = book();
    let c = Catalog::embedded();
    let r = entry(&c, "revenant");
    let mut p = Progress::default();
    let u = p.meet(&b, &r).unwrap();
    assert_eq!(b.line(&u), "Книга охотника: Латник-мертвец — Встречен");
    assert!(p.meet(&b, &r).is_none());
    let lines: Vec<String> = (0..15).filter_map(|_| p.kill(&b, &r, 1.0)).map(|u| b.line(&u)).collect();
    assert_eq!(lines, ["Книга охотника: Латник-мертвец — Убит", "Книга охотника: Латник-мертвец — Изучен", "Книга охотника: Латник-мертвец — Знаток"]);
    // A kill without a meeting first opens straight to «Убит».
    let brute = entry(&c, "brute");
    let mut q = Progress::default();
    assert_eq!(q.kill(&b, &brute, 1.0).unwrap().stage, "Убит");
    assert!(q.kill(&b, &brute, 1.0).is_none());
    assert_eq!(q.kill(&b, &brute, 1.0).unwrap().stage, "Изучен");
}

#[test]
fn altar_entries_open_once_each() {
    let b = book();
    let c = Catalog::embedded();
    let a = AltarFile::embedded();
    let mut p = Progress::default();
    assert_eq!(p.altar_base(&b).unwrap().title, "Алтарь: основа");
    assert!(p.altar_base(&b).is_none());
    let ids: Vec<String> = p.altar_levels(2).into_iter().map(|u| u.id).collect();
    assert_eq!(ids, ["level_1", "level_2"]);
    let ids: Vec<String> = p.altar_levels(9).into_iter().map(|u| u.id).collect();
    assert_eq!(ids, ["level_3", "level_4"]);
    assert!(p.altar_levels(4).is_empty());
    assert_eq!(p.banner(&a, "weight").unwrap().title, "Знамя тяжести");
    assert!(p.banner(&a, "weight").is_none());
    assert!(p.banner(&a, "nonsense").is_none());
    let tried: Vec<String> = p.event_tried(&b, &c, "test_zombies").into_iter().map(|u| format!("{} {}", u.title, u.stage)).collect();
    assert_eq!(tried, ["Оборона алтаря открыто", "Тестовый ивент первого среза описание"]);
    assert!(p.event_tried(&b, &c, "test_zombies").is_empty());
    let won: Vec<String> = p.event_won(&b, &c, "test_zombies").into_iter().map(|u| u.stage).collect();
    assert_eq!(won, ["волны и награда"]);
    assert!(p.event_won(&b, &c, "test_zombies").is_empty());
    // A win with no try seen first opens all three.
    let mut q = Progress::default();
    assert_eq!(q.event_won(&b, &c, "test_zombies").len(), 3);
    assert_eq!(q.events["test_zombies"], EventProgress { tried: true, won: true });
}

#[test]
fn items_in_hand_open_the_base_or_a_banner() {
    let a = AltarFile::embedded();
    let summon = vec!["soulscraft:rotten_horn".to_owned()];
    assert_eq!(held("soulscraft:altar_core", &a, &summon), Some(Held::Base));
    assert_eq!(held("soulscraft:altar_slab", &a, &summon), Some(Held::Base));
    assert_eq!(held("soulscraft:rotten_horn", &a, &summon), Some(Held::Base));
    assert_eq!(held("soulscraft:banner_horde", &a, &summon), Some(Held::Banner("horde".into())));
    assert_eq!(held("soulscraft:banner_kin_water", &a, &summon), Some(Held::Banner("kin_water".into())));
    assert_eq!(held("minecraft:stone", &a, &summon), None);
    assert_eq!(held(ITEM, &a, &summon), None);
}

#[test]
fn the_killer_and_near_recent_hitters_share_a_kill() {
    let b = book();
    let (a, bb, far, late, gone, killer) = (1u128, 2u128, 3u128, 4u128, 5u128, 6u128);
    let died = [0.0, 100.0, 0.0];
    let at: HashMap<u128, V3> = [(a, [10.0, 100.0, 0.0]), (bb, [0.0, 100.0, 31.9]), (far, [40.0, 100.0, 0.0]), (late, [1.0, 100.0, 0.0]), (killer, [500.0, 100.0, 0.0])].into();
    let now = 1000;
    let hits = [
        Hit { player: a, tick: now - 20 },
        Hit { player: bb, tick: now - 200 },
        Hit { player: far, tick: now - 5 },
        Hit { player: late, tick: now - 201 },
        Hit { player: gone, tick: now - 1 },
        Hit { player: killer, tick: now },
    ];
    // The killer counts wherever they stand; others in 32 blocks and 10 seconds.
    assert_eq!(credited(&b, Some(killer), &hits, now, died, &at), [killer, a, bb]);
    assert_eq!(credited(&b, None, &hits, now, died, &at), [a, bb]);
    assert!(credited(&b, None, &[], now, died, &at).is_empty());
    assert!(owner_credited(&b, Some([0.0, 100.0, 32.0]), died));
    assert!(!owner_credited(&b, Some([0.0, 100.0, 32.5]), died));
    assert!(!owner_credited(&b, None, died));
    assert_eq!(weight(&b, false), 1.0);
    assert_eq!(weight(&b, true), 0.25);
}

#[test]
fn the_hit_log_keeps_each_players_last_hit_for_the_window() {
    let mut log = HitLog::default();
    assert!(log.is_empty());
    log.add(7, 1, 10);
    log.add(7, 1, 30);
    log.add(7, 2, 40);
    log.add(8, 1, 5);
    assert_eq!(log.len(), 2);
    log.prune(220, 200);
    assert_eq!(log.len(), 1, "mob 8's only hit is older than the window");
    let hits = log.take(7);
    assert_eq!(hits, [Hit { player: 1, tick: 30 }, Hit { player: 2, tick: 40 }]);
    assert!(log.take(7).is_empty());
}

#[test]
fn turret_reports_read_as_rustcraft_writes_them() {
    let owner = 0x26a5132a990442e59cfd2cccd5083fa8u128;
    assert_eq!(turret_report("pct-machine kill 26a5132a990442e59cfd2cccd5083fa8 42 soulscraft:revenant"), Some((owner, 42, "soulscraft:revenant".into())));
    assert_eq!(turret_report("/pct-machine kill 00000000000000000000000000000001 7 minecraft:zombie"), Some((1, 7, "minecraft:zombie".into())));
    assert_eq!(turret_report("pct-machine break 1 minecraft:overworld 1 2 3 x axe"), None);
    assert_eq!(turret_report("pct-machine kill zz 7 minecraft:zombie"), None);
    assert_eq!(turret_report("pct-machine kill 1 7"), None);
    assert_eq!(turret_report("say hi"), None);
}

#[test]
fn progress_survives_its_saved_text_and_the_message() {
    let b = book();
    let c = Catalog::embedded();
    let mut p = Progress::default();
    p.kill(&b, &entry(&c, "stalker"), 0.25);
    p.kill(&b, &entry(&c, "brute"), 1.0);
    p.altar_levels(2);
    p.banner(&AltarFile::embedded(), "calm");
    p.event_won(&b, &c, "test_zombies");
    let text = p.to_json();
    assert_eq!(Progress::from_json(&text).unwrap(), p);
    assert_eq!(Progress::from_json("{}").unwrap(), Progress::default());
    assert!(Progress::from_json("{\"mobs\": 3}").is_err());
    let m = BookMsg { open: true, progress: p.clone() };
    assert_eq!(decode(&encode(&m)), Some(m));
    assert_eq!(decode(b"x"), None);
    // Kills keep two decimals: no float drift after many quarters.
    let mut q = Progress::default();
    for _ in 0..40 {
        q.kill(&b, &entry(&c, "stalker"), 0.25);
    }
    assert_eq!(q.mob("stalker").kills, 10.0);
}

#[test]
fn strips_follow_the_attack_phases() {
    let c = Catalog::embedded();
    let (_, r) = c.get("revenant").unwrap();
    let overhead = r["attacks"].as_array().unwrap().iter().find(|a| a["id"] == "overhead").unwrap();
    let s = book_page::strip(overhead).unwrap();
    assert_eq!(s.segs, [(Seg::Windup, 10), (Seg::Hold, 8), (Seg::Hit, 2), (Seg::Window, 16)]);
    assert_eq!(s.ticks(), 36);
    assert_eq!((s.seg_at(0), s.seg_at(9), s.seg_at(10), s.seg_at(18), s.seg_at(20), s.seg_at(36)), (Some(Seg::Windup), Some(Seg::Windup), Some(Seg::Hold), Some(Seg::Hit), Some(Seg::Window), Some(Seg::Windup)));
    assert_eq!(s.clip, "attack_overhead");
    assert!(s.tell.contains("над головой"));
    // No hold: three parts. A grab (follows) and an attack of no ticks: none.
    let thrust = r["attacks"].as_array().unwrap().iter().find(|a| a["id"] == "thrust").unwrap();
    assert_eq!(book_page::strip(thrust).unwrap().segs.len(), 3);
    assert!(book_page::strip(&json!({"follows": "ambush", "windup": 3})).is_none());
    assert!(book_page::strip(&json!({"name": "x"})).is_none());
    assert_eq!(book_page::Strip { name: String::new(), clip: String::new(), tell: String::new(), segs: vec![] }.seg_at(3), None);
    assert_eq!(Seg::Window.label(), "окно игрока");
}

fn texts(p: &book_page::Page) -> String {
    p.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n")
}

#[test]
fn each_stage_opens_what_the_table_says() {
    let b = book();
    let c = Catalog::embedded();
    let t = trophies();
    let (e, m) = c.get("revenant").unwrap().clone();
    let mut p = Progress::default();
    let page = |p: &Progress| book_page::mob_page(&b, &c, &t, &e, &m, p);
    let locked = page(&p);
    assert_eq!((locked.title.as_str(), locked.silhouette, locked.moves, locked.strips.len()), ("???", true, false, 0));
    assert!(!texts(&locked).contains("Латник"));
    p.meet(&b, &e);
    let met = page(&p);
    assert_eq!((met.title.as_str(), met.silhouette, met.moves), ("Латник-мертвец", false, false));
    assert!(texts(&met).contains("Где водится"));
    assert!(texts(&met).contains("ещё 1 убийств"));
    assert!(!texts(&met).contains("Здоровье"));
    p.kill(&b, &e, 1.0);
    let killed = page(&p);
    assert!(killed.moves && killed.strips.is_empty());
    assert!(texts(&killed).contains("Здоровье 30 · урон 4 · броня 6"));
    assert!(texts(&killed).contains("Атаки: Укол, Рубящий удар, Задержанный удар сверху, Удар щитом"));
    for _ in 0..4 {
        p.kill(&b, &e, 1.0);
    }
    let studied = page(&p);
    assert_eq!(studied.strips.len(), 4);
    assert!(!texts(&studied).contains("Тактика"));
    for _ in 0..10 {
        p.kill(&b, &e, 1.0);
    }
    let expert = page(&p);
    let all = texts(&expert);
    assert_eq!(expert.stage, EXPERT);
    for want in ["Тактика", "Добыча", "голова — 1%", "Рост по силе P", "здоровье 30 → 57"] {
        assert!(all.contains(want), "{want} missing from:\n{all}");
    }
    assert!(!all.contains("До ступени"));
    assert!(expert.lines.iter().any(|l| l.tone == Tone::Head));
    // The elite brute's head chance, and a mob in an event's waves.
    let (brute, bm) = c.get("brute").unwrap().clone();
    let mut q = Progress::default();
    for _ in 0..8 {
        q.kill(&b, &brute, 1.0);
    }
    let bp = book_page::mob_page(&b, &c, &t, &brute, &bm, &q);
    assert!(texts(&bp).contains("голова — 3%"));
}

#[test]
fn the_event_link_names_waves_and_vanilla_pages_have_no_head() {
    let b = book();
    let t = trophies();
    let root = json!({
        "mobs": [],
        "vanilla": [{"id": "zombie", "name": "Зомби", "keys": ["minecraft:zombie"], "stats": {"health": 20, "damage": 3}, "behavior": ["Отступает после удара"], "attacks": [{"id": "lunge", "name": "Выпад", "clip": "", "windup": 10, "hit": 2, "recover": 8}]}],
        "events": [{"id": "e", "name": "Ивент", "waves": [{"wave": 1, "mobs": [{"type": "minecraft:zombie", "count": 4}]}, {"wave": 2, "mobs": [{"type": "minecraft:skeleton", "count": 1}]}, {"wave": 3, "mobs": [{"type": "minecraft:zombie", "count": 2}]}]}]
    });
    let c = Catalog::from_value(&root);
    let (e, m) = c.get("zombie").unwrap().clone();
    let mut p = Progress::default();
    for _ in 0..15 {
        p.kill(&b, &e, 1.0);
    }
    let page = book_page::mob_page(&b, &c, &t, &e, &m, &p);
    let all = texts(&page);
    assert!(all.contains("Ивент: волны 1, 3"), "{all}");
    assert!(!all.contains("голова"));
    assert_eq!(page.strips[0].name, "Выпад");
}

#[test]
fn the_altar_page_hides_what_is_not_found() {
    let b = book();
    let a = AltarFile::embedded();
    let mut p = Progress::default();
    let page = book_page::altar_page(&b, &a, &p);
    let all = texts(&page);
    assert!(all.contains("Алтарь: ???"));
    assert!(all.contains("Уровень 4: ???"));
    assert!(all.contains(&format!("Не найдено знамён: {}", a.modifiers.len())));
    assert!(!all.contains("лабиринт"));
    p.altar_base(&b);
    p.altar_levels(2);
    p.banner(&a, "horde");
    p.event_tried(&b, &Catalog::embedded(), "test_zombies");
    let all = texts(&book_page::altar_page(&b, &a, &p));
    assert!(all.contains("кольцами"));
    assert!(all.contains("Уровень 2: кольцо 5×5, углы: колонна"));
    assert!(all.contains("Уровень 3: ???"));
    assert!(all.contains("Знамя орды: В волнах на 25% больше врагов"));
    assert!(all.contains("лабиринт"));
    assert_eq!(book_page::piece_name("soulscraft:altar_obelisk"), "обелиск");
    assert_eq!(book_page::piece_name("soulscraft:other"), "other");
}

#[test]
fn the_events_page_opens_the_description_then_the_waves() {
    let b = book();
    let c = Catalog::embedded();
    let t = trophies();
    let mut p = Progress::default();
    let all = texts(&book_page::events_page(&b, &c, &t, &p));
    assert!(all.contains("Ивент: ???"));
    p.event_tried(&b, &c, "test_zombies");
    let all = texts(&book_page::events_page(&b, &c, &t, &p));
    assert!(all.contains("Тестовый ивент первого среза"));
    assert!(all.contains("Гнилой горн"));
    assert!(all.contains("после первой победы"));
    assert!(!all.contains("Волна 1"));
    p.event_won(&b, &c, "test_zombies");
    let all = texts(&book_page::events_page(&b, &c, &t, &p));
    assert!(all.contains("Волна 1:"), "{all}");
    assert!(all.contains("Награда:"));
}
