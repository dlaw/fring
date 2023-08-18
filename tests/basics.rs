use fring::Buffer;

#[test]
#[should_panic]
fn zero_length() {
    let _b: Buffer<0> = Buffer::new();
}

#[test]
#[should_panic]
fn not_power_of_two_length() {
    let _b: Buffer<3> = Buffer::new();
}

#[test]
fn read_and_write() {
    let mut b: Buffer<2> = Buffer::new();
    let (mut p, mut c) = b.split();
    let mut w = p.write(usize::MAX);
    assert!(w.len() == 2);
    w[0] = 22;
    w[1] = 23;
    let r = c.read(usize::MAX);
    assert!(r.len() == 0);
    drop(w);
    drop(r);
    let r = c.read(usize::MAX);
    assert!(r.len() == 2);
    assert!(r[0] == 22);
    assert!(r[1] == 23);
}

#[test]
fn wrap_around() {
    let mut b: Buffer<32> = Buffer::new();
    let (mut p, mut c) = b.split();
    for i in 0..33 {
        let w = p.write(i);
        let mut len = w.len();
        drop(w);
        if len != i {
            let w = p.write(i - len);
            len += w.len();
        }
        assert!(len == i);
        let r = c.read(usize::MAX);
        let mut len = r.len();
        drop(r);
        if len != i {
            let r = c.read(usize::MAX);
            len += r.len();
        }
        assert!(len == i);
    }
}

#[test]
fn flush() {
    let mut b: Buffer<8> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(5);
    c.flush();
    assert!(c.read(usize::MAX).len() == 0);
}

#[test]
fn empty_size() {
    let mut b: Buffer<8> = Buffer::new();
    let (mut p, mut c) = b.split();
    assert!(p.empty_size() == 8);
    p.write(5);
    assert!(p.empty_size() == 3);
    p.write(5);
    assert!(p.empty_size() == 0);
    c.read(5);
    assert!(p.empty_size() == 5);
    c.read(5);
    assert!(p.empty_size() == 8);
}

#[test]
fn data_size() {
    let mut b: Buffer<8> = Buffer::new();
    let (mut p, mut c) = b.split();
    assert!(c.data_size() == 0);
    p.write(5);
    assert!(c.data_size() == 5);
    p.write(5);
    assert!(c.data_size() == 8);
    c.read(5);
    assert!(c.data_size() == 3);
    c.read(5);
    assert!(c.data_size() == 0);
}