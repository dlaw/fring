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
        idx+=1;
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
        // Retry until 3 bytes can be written (buffer may be full or the write
        // may straddle the buffer boundary).
        let mut region = loop {
            match p.write(3) {
                Ok(region) => break region,
                Err(_) => {
                    std::thread::yield_now();
                },
            }
        };

        region[0] = data[0];
        region[1] = data[1];
        region[2] = data[2];

        println!("write \"{:?} {:?} {:?}\"", region[0], region[1], region[2]);
    }
}

fn consumer(mut c: fring::Consumer<T, N>) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(60));
        match c.read(usize::MAX) {
            Ok(r) => {
                if r.len() == 0 {
                    // buffer is empty
                    break;
                }
                if r.len() >= 3 {
                    for i in r.iter() {
                        println!("read \"{:?}\"", i);
                    }
                }
            }
            Err(_) => {
                // buffer is empty
                break;
            }
        }
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
