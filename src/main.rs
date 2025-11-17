use std::io;
use std::thread;

use memmap2::{Advice, Mmap};
use rustc_hash::FxHashMap;

type HashMap = FxHashMap::<Vec<u8>, Station>;

// One station.
#[derive(Clone)]
struct Station {
    min: i32,
    max: i32,
    total: i64,
    count: u32,
}

impl Station {
    fn new(value: i32) -> Station {
        Station{
            min: value,
            max: value,
            total: value as i64,
            count: 1
        }
    }

    fn add(&mut self, other: &Station) {
        if other.min < self.min {
            self.min = other.min;
        }
        if other.max > self.max {
            self.max = other.max;
        }
        self.total += other.total;
        self.count += other.count;
    }

    fn update(&mut self, value: i32) {
        if value > self.max {
            self.max= value;
        } else if value < self.min {
            self.min = value;
        }
        self.total += value as i64;
        self.count += 1;
    }
}

// Divide a slice of bytes into nsegs segments, on newline boundaries.
fn segments(data: &[u8], nsegs: usize) -> Vec<&[u8]> {
    let mut segs = Vec::new();
    let seg_size = data.len() / nsegs;
    let mut offset = 0;
    for i in 1..=nsegs {
        let mut end = if i < nsegs { i * seg_size } else { data.len() - 1 };
        while data[end] != b'\n' {
            end += 1;
        }
        end += 1;
        segs.push(&data[offset..end]);
        offset = end;
    }
    segs
}

// Parse a floating point number with exactly one digit after the decimal point.
// Returns the value * 10.
fn parsenum(number: &[u8]) -> i32 {
    let neg = (number[0] == b'-') as usize;
    let val = &number[neg..number.len() -2]
        .iter()
        .fold(0, |tot, &val| tot * 10 + (val - b'0') as i32) * 10
        + (number[number.len()-1] - b'0') as i32;
    if neg == 0 { val } else { -val }
}

// Process one segment.
fn worker(data: &[u8]) -> HashMap {
    let mut hm = HashMap::default();
    let mut splitter = data.split(|&c| c == b';' || c == b'\n');
    loop {
        let name = splitter.next().unwrap();
        if name.len() == 0 {
            break;
        }
        let number = splitter.next().unwrap();
        let value = parsenum(number);
        if let Some(entry) = hm.get_mut(name) {
            entry.update(value);
        } else {
            hm.insert(name.to_owned(), Station::new(value));
        }
    }
    hm
}

// Combine the values in all hashmaps into one.
fn combine(hms: Vec<HashMap>) -> HashMap {
    let mut hm = HashMap::default();
    for h in &hms {
        for (name, entry) in h.iter() {
            if let Some(hmentry) = hm.get_mut(name) {
                hmentry.add(entry);
            } else {
                hm.insert(name.to_owned(), entry.clone());
            }
        }
    }
    hm
}

// Print report on stdout.
fn report(hm: &HashMap) {
    let mut names = hm.keys()
        .map(|s| String::from_utf8(s.to_vec()).expect("utf-8"))
        .collect::<Vec<_>>();
    names.sort();
    print!("{{");
    for (idx, name) in names.iter().enumerate() {
        let entry = &hm[name.as_bytes()];
        if idx != 0 {
            print!(", ");
        }
        let mean = ((entry.total as f64) / (entry.count as f64)) / 10.0;
        print!("{}={:.1}/{:.1}/{:.1}", name, entry.min as f64 / 10.0, mean, entry.max as f64 / 10.0);
    }
    println!("}}");
}

fn main() {
    // Mmap stdin.
    let file = io::stdin();
    let mmap = unsafe { Mmap::map(&file) }.expect("Mmap::map");
    let _ = mmap.advise(Advice::Sequential);

    // Split up into as many segments as we have cpus.
    let ncpus = thread::available_parallelism().expect("ncpus").into();
    let segs = segments(&mmap[..], ncpus);

    let results = thread::scope(|s| {
        let mut handles = Vec::new();

        // Spawn threads.
        for seg in &segs {
            let handle = s.spawn(move || {
                worker(seg)
            });
            handles.push(handle);
        }

        // Await threads and collect results.
        handles
            .drain(..)
            .map(|h| h.join().expect("join"))
            .collect::<Vec<_>>()
    });

    let hm = combine(results);
    report(&hm);
}
