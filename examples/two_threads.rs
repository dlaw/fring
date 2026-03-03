use std::time::Instant;

const N: usize = 8;

fn producer(mut p: fring::Producer<u8, N>) {
    let mut index = 'a' as u8;
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        // Retry until 3 items can be written (buffer may be full or the write
        // may straddle the buffer boundary).
        let mut w = loop {
            match p.write(3) {
                Ok(r) => break r,
                Err(_) => std::thread::yield_now(),
            }
        };
        for i in 0..w.len() {
            w[i] = index;
            index += 1;
        }
        println!("write \"{}\"", std::str::from_utf8(&*w).unwrap());
    }
}

fn consumer(mut c: fring::Consumer<u8, N>) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(60));
        match c.read(usize::MAX) {
            Ok(r) => {
                if r.len() == 0 {
                    // buffer is empty
                    break;
                }
                println!("            read \"{}\"", std::str::from_utf8(&*r).unwrap());
            }
            Err(_) => break,
        }
    }
}

fn main() {
    let start = Instant::now();
    let mut b = fring::Buffer::<u8, N>::new();
    let (p, c) = b.split();
    std::thread::scope(|s| {
        s.spawn(|| {
            producer(p);
        });
        consumer(c);
    });
    println!("Elapsed: {:?}", start.elapsed());
}
