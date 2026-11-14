//! What the server tells the clients about its SoulsCraft mobs, as mod messages (API 40) on
//! [`STATE`]: for each mob whose clip or flags changed, the clip and the server tick it
//! started. The client counts the clip's ticks from there, so one message covers a whole attack.

/// The channel (`mod_id:name`).
pub const STATE: &str = "soulscraft:state";
const VERSION: u8 = 1;
/// Flag bits.
pub const RAGE: u8 = 1;
pub const GUARD: u8 = 2;
pub const DEBUG: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Note {
    /// The mob's network id (the id `native_mob_submit` gives).
    pub id: u32,
    /// The clip; empty: the client's own walk or idle.
    pub clip: String,
    /// The server tick the clip started.
    pub start: i64,
    pub flags: u8,
    /// The bog slime's size (1, 2, 4); 0 for the others.
    pub size: u8,
    /// A label for the debug view (`overhead:hold 12`); empty unless debug is on.
    pub label: String,
}

/// One message: the server's tick now, then the notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Batch {
    pub now: i64,
    pub notes: Vec<Note>,
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    let b = &s.as_bytes()[..s.len().min(255)];
    out.push(b.len() as u8);
    out.extend_from_slice(b);
}

/// A message holds at most 30 000 bytes (API 40); batches are cut below that.
pub const MAX_BYTES: usize = 29_000;

fn put_note(out: &mut Vec<u8>, note: &Note) {
    out.extend_from_slice(&note.id.to_le_bytes());
    put_str(out, &note.clip);
    out.extend_from_slice(&note.start.to_le_bytes());
    out.push(note.flags);
    out.push(note.size);
    put_str(out, &note.label);
}

/// The batch as messages of at most [`MAX_BYTES`] each (one when it is small).
pub fn encode(b: &Batch) -> Vec<Vec<u8>> {
    let head = || {
        let mut out = vec![VERSION];
        out.extend_from_slice(&b.now.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    };
    let finish = |mut out: Vec<u8>, n: u16| {
        out[9..11].copy_from_slice(&n.to_le_bytes());
        out
    };
    let mut messages = Vec::new();
    let (mut out, mut n) = (head(), 0u16);
    for note in &b.notes {
        let mut one = Vec::new();
        put_note(&mut one, note);
        if n > 0 && out.len() + one.len() > MAX_BYTES {
            messages.push(finish(std::mem::replace(&mut out, head()), n));
            n = 0;
        }
        out.extend_from_slice(&one);
        n += 1;
    }
    messages.push(finish(out, n));
    messages
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        if self.0.len() < n {
            return None;
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Some(a)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn i64(&mut self) -> Option<i64> {
        self.take(8).map(|b| i64::from_le_bytes(b.try_into().unwrap()))
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
}

pub fn decode(data: &[u8]) -> Option<Batch> {
    let mut r = Reader(data);
    if r.u8()? != VERSION {
        return None;
    }
    let now = r.i64()?;
    let n = u16::from_le_bytes(r.take(2)?.try_into().ok()?);
    let mut notes = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let id = u32::from_le_bytes(r.take(4)?.try_into().ok()?);
        let clip = r.str()?;
        let start = r.i64()?;
        let flags = r.u8()?;
        let size = r.u8()?;
        let label = r.str()?;
        notes.push(Note { id, clip, start, flags, size, label });
    }
    r.0.is_empty().then_some(Batch { now, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch() -> Batch {
        Batch {
            now: 123_456,
            notes: vec![
                Note { id: 7, clip: "attack_overhead".into(), start: 123_450, flags: GUARD, size: 0, label: "overhead:hold 6".into() },
                Note { id: 9, clip: String::new(), start: 1, flags: RAGE | DEBUG, size: 4, label: String::new() },
            ],
        }
    }

    #[test]
    fn round_trip() {
        let m = encode(&batch());
        assert_eq!(m.len(), 1);
        assert_eq!(decode(&m[0]), Some(batch()));
        assert_eq!(decode(&encode(&Batch::default())[0]), Some(Batch::default()));
    }

    #[test]
    fn broken_messages_are_refused() {
        let bytes = encode(&batch()).remove(0);
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(decode(&longer), None);
        let mut other = bytes;
        other[0] = 99;
        assert_eq!(decode(&other), None);
        assert_eq!(decode(&[]), None);
    }

    #[test]
    fn a_big_batch_is_cut_into_messages_that_fit() {
        let note = Note { id: u32::MAX, clip: "x".repeat(255), start: 0, flags: 0, size: 0, label: "y".repeat(400) };
        let b = Batch { now: 5, notes: vec![note; 200] };
        let messages = encode(&b);
        assert!(messages.len() > 1);
        let mut back = Vec::new();
        for m in &messages {
            assert!(m.len() <= MAX_BYTES, "{}", m.len());
            let d = decode(m).unwrap();
            assert_eq!(d.now, 5);
            back.extend(d.notes);
        }
        assert_eq!(back.len(), 200);
        assert_eq!(back[0].label.len(), 255);
    }
}
