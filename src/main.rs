use std::hash::{BuildHasher, Hasher};
use std::io;
use std::os::fd::AsRawFd;
use std::thread;

const PRIME_1: u64 = 0x9E3779B185EBCA87;
const PRIME_2: u64 = 0xC2B2AE3D27D4EB4F;
const PRIME_3: u64 = 0x165667B19E3779F9;
const PRIME_4: u64 = 0x85EBCA77C2B2AE63;
const PRIME_5: u64 = 0x27D4EB2F165667C5;

#[inline(always)]
fn get_uint<T: Copy>(input: &[u8]) -> T {
    debug_assert!(std::mem::size_of::<T>() > 0);
    debug_assert!(std::mem::size_of::<T>() <= input.len());

    unsafe {
        std::ptr::read_unaligned(input.as_ptr() as *const T)
    }
}

#[derive(Default)]
struct XXHasherBuilder;
struct XXHasher(u64);

impl BuildHasher for XXHasherBuilder {
    type Hasher = XXHasher;

    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        XXHasher(0)
    }
}

impl Hasher for XXHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline(always)]
    fn write(&mut self, mut data: &[u8]) {
        //
        // This is a variation on the xxhash_64 algoritm. It only implements
        // the hash algorithm from xxhash for data < 32 bytes. The original hash
        // has a faster algorithm for data >= 32 bytes. This will still handle
        // data >= 32 bytes, only a bit slower than the original.
        //
        let mut input = PRIME_5.wrapping_add(data.len() as u64);

        while data.len() >= 8 {
            input ^= get_uint::<u64>(data).to_le().wrapping_mul(PRIME_2).rotate_left(31).wrapping_mul(PRIME_1);
            input = input.rotate_left(27).wrapping_mul(PRIME_1).wrapping_add(PRIME_4);
            data = &data[8..];
        }

        while data.len() >= 4 {
            input ^= (get_uint::<u32>(data).to_le() as u64).wrapping_mul(PRIME_1);
            input = input.rotate_left(23).wrapping_mul(PRIME_2).wrapping_add(PRIME_3);
            data = &data[4..];
        }

        for byte in data.iter() {
            input ^= (*byte as u64).wrapping_mul(PRIME_5);
            input = input.rotate_left(11).wrapping_mul(PRIME_1);
        }

        // The original xxhash_64 algorithm runs a finalizer over 'input'. We don't bother.
        self.0 = input;
    }
}

type HashMap = std::collections::HashMap::<Vec<u8>, Station, XXHasherBuilder>;


// One station.
#[derive(Clone)]
struct Station {
    min: isize,
    max: isize,
    total: isize,
    count: usize,
}

impl Station {
    fn new(value: isize) -> Station {
        Station{
            min: value,
            max: value,
            total: value,
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

    fn update(&mut self, value: isize) {
        if value > self.max {
            self.max= value;
        } else if value < self.min {
            self.min = value;
        }
        self.total += value;
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
fn parsenum(number: &[u8]) -> isize {
    let neg = (number[0] == b'-') as usize;
    let val = &number[neg..number.len() -2]
        .iter()
        .fold(0, |tot, &val| tot * 10 + (val - b'0') as isize) * 10
        + (number[number.len()-1] - b'0') as isize;
    if neg == 0 { val } else { val.wrapping_neg() }
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

fn mmap<'a>(f: &'a impl AsRawFd) -> io::Result<&'a [u8]> {
    unsafe {
        let len = libc::lseek(f.as_raw_fd(), 0, libc::SEEK_END);
        if len < 0 {
            return Err(io::Error::last_os_error());
        }
        let ptr = libc::mmap(
            std::ptr::null_mut(),
            len as libc::size_t,
            libc::PROT_READ,
            libc::MAP_SHARED,
            f.as_raw_fd(),
            0,
        );
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        if libc::madvise(ptr, len as libc::size_t, libc::MADV_SEQUENTIAL) != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(std::slice::from_raw_parts(ptr as *const u8, len as usize))
    }
}

fn main() {
    // Mmap stdin.
    let file = io::stdin();
    let map = mmap(&file).expect("Mmap::map");

    // Split up into as many segments as we have cpus.
    let ncpus = thread::available_parallelism().expect("ncpus").into();
    let segs = segments(&map[..], ncpus);

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
