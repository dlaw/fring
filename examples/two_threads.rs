const N: usize = 8;

fn producer(mut p: fring::Producer<N>) {
    let mut index = 'a' as u8;
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        let mut w = p.write(3);
        for i in 0..w.len() {
            w[i] = index;
            index += 1;
        }
        println!("write \"{}\"", std::str::from_utf8(&*w).unwrap());
    }
}

fn consumer(mut c: fring::Consumer<N>) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(60));
        let r = c.read(usize::MAX);
        if r.len() == 0 {  // buffer is empty
            break;
        }
        println!("            read \"{}\"", std::str::from_utf8(&*r).unwrap());
    }
}

fn main() {
    let mut b = fring::Buffer::<N>::new();
    let (p, c) = b.split();
    std::thread::scope(|s| {
        s.spawn(|| {
            producer(p);
        });
        consumer(c);
    });
}