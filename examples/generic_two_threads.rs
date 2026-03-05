const N: usize = 8;
type T = Test;

#[derive(Debug, Copy, Clone, Default)]
struct Test {
    field: u32,
    another_field: f32,
}

fn producer(mut p: fring::Producer<T, N>) {
    println!("Producer thread started");
    let mut idx = 0;
    loop {
        idx += 1;
        std::thread::sleep(std::time::Duration::from_millis(25));
        let data = [
            Test {
                field: idx,
                another_field: idx as f32 + 3.0,
            },
            Test {
                field: idx + 1,
                another_field: (idx + 1) as f32 + 4.0,
            },
            Test {
                field: idx + 2,
                another_field: (idx + 2) as f32 + 5.0,
            },
        ];
        let mut w = p.write(3);
        for i in 0..w.len() {
            w[i] = data[i];
        }

        println!("write \"{:?} {:?} {:?}\"", data[0], data[1], data[2]);
    }
}

fn consumer(mut c: fring::Consumer<T, N>) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(60));
        let r = c.read(usize::MAX);
        if r.len() == 0 {
            // buffer is empty
            break;
        }
        println!("            read \"{:?} {:?} {:?}\"", r[0], r[1], r[2]);
    }
}

fn main() {
    let mut b = fring::Buffer::<T, N>::new();
    let (p, c) = b.split();
    std::thread::scope(|s| {
        s.spawn(|| {
            producer(p);
        });
        consumer(c);
    });
}
