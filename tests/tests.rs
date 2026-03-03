use fring::Buffer;

#[test]
fn read_and_write() {
    let mut b: Buffer<u8, 2> = Buffer::new();
    let (mut p, mut c) = b.split();
    let mut w = p.write(2).unwrap();
    assert!(w.len() == 2);
    w[0] = 22;
    w[1] = 23;
    let r = c.read(usize::MAX).unwrap();
    assert!(r.len() == 0);
    drop(w);
    drop(r);
    let r = c.read(usize::MAX).unwrap();
    assert!(r.len() == 2);
    assert!(r[0] == 22);
    assert!(r[1] == 23);
}

#[test]
fn producer_consumer() {
    let b: Buffer<u8, 2> = Buffer::new();
    let mut p = unsafe { b.producer() };
    let mut c = unsafe { b.consumer() };
    p.write(1).unwrap()[0] = 22;
    assert!(c.read(1).unwrap()[0] == 22);
}

#[test]
fn wrap_around() {
    let mut b: Buffer<u8, 32> = Buffer::new();
    let (mut p, mut c) = b.split();
    for i in 0..33 {
        let mut written = 0;
        while written < i {
            let remaining = i - written;
            // Try to write `remaining` bytes. The Result temporary (and its
            // Region, if Ok) are dropped at the end of this statement,
            // releasing the mutable borrow on `p`.
            let ok = p.write(remaining).is_ok();
            if ok {
                written += remaining;
            } else {
                // Not enough contiguous space; write 1 byte to advance past wrap
                p.write(1).unwrap();
                written += 1;
            }
        }
        assert!(written == i);
        let r = c.read(usize::MAX).unwrap();
        let mut len = r.len();
        drop(r);
        if len != i {
            let r = c.read(usize::MAX).unwrap();
            len += r.len();
        }
        assert!(len == i);
    }
}

#[test]
fn flush() {
    let mut b: Buffer<u8, 8> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(5).unwrap();
    c.flush();
    assert!(c.read(usize::MAX).unwrap().len() == 0);
}

#[test]
fn empty_size() {
    let mut b: Buffer<u8, 8> = Buffer::new();
    let (mut p, mut c) = b.split();
    assert!(p.empty_size() == 8);
    p.write(5).unwrap();
    assert!(p.empty_size() == 3);
    p.write(3).unwrap();
    assert!(p.empty_size() == 0);
    c.read(5).unwrap();
    assert!(p.empty_size() == 5);
    c.read(3).unwrap();
    assert!(p.empty_size() == 8);
}

#[test]
fn data_size() {
    let mut b: Buffer<u8, 8> = Buffer::new();
    let (mut p, mut c) = b.split();
    assert!(c.data_size() == 0);
    p.write(5).unwrap();
    assert!(c.data_size() == 5);
    p.write(3).unwrap();
    assert!(c.data_size() == 8);
    c.read(5).unwrap();
    assert!(c.data_size() == 3);
    c.read(3).unwrap();
    assert!(c.data_size() == 0);
}

#[test]
fn consume() {
    let mut b: Buffer<u8, 4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(1).unwrap()[0] = 1;
    p.write(1).unwrap()[0] = 2;
    p.write(1).unwrap()[0] = 3;
    let mut r = c.read(usize::MAX).unwrap();
    assert!(r.len() == 3);
    r.consume(1);
    assert!(r.len() == 2);
    assert!(r[0] == 2);
    assert!(r[1] == 3);
}

#[test]
#[should_panic]
fn bad_consume() {
    let mut b: Buffer<u8, 4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(2).unwrap();
    let mut r = c.read(usize::MAX).unwrap();
    r.consume(3);
}

#[test]
fn partial_drop() {
    let mut b: Buffer<u8, 4> = Buffer::new();
    let (mut p, c) = b.split();
    let w = p.write(3).unwrap();
    w.partial_drop(1);
    assert!(c.data_size() == 1);
}

#[test]
#[should_panic]
fn bad_partial_drop() {
    let mut b: Buffer<u8, 4> = Buffer::new();
    let (mut p, _) = b.split();
    let w = p.write(2).unwrap();
    w.partial_drop(3);
}

#[test]
fn zero_sized_ops() {
    let mut b: Buffer<u8, 4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(1).unwrap();
    let mut r = c.read(0).unwrap();
    r.consume(0);
    r.partial_drop(0);
    assert!(c.data_size() == 1);
    p.write(0).unwrap();
    assert!(p.empty_size() == 3);
}
