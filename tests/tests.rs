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
fn producer_consumer() {
    let b: Buffer<2> = Buffer::new();
    let mut p = unsafe { b.producer() };
    let mut c = unsafe { b.consumer() };
    p.write(1)[0] = 22;
    assert!(c.read(1)[0] == 22);
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

#[test]
fn read_ref() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(1)[0] = 0;
    p.write(1)[0] = 1;
    p.write(1)[0] = 2;
    p.write(1)[0] = 3;
    c.read(1);
    p.write(1)[0] = 4;
    let mut r: [u8; 2] = [0, 0];
    assert!(unsafe { c.read_ref(&mut r) }.is_ok());  // not wrapped around
    assert!(r == [1, 2]);
    p.write(1)[0] = 5;
    assert!(unsafe { c.read_ref(&mut r) }.is_ok());  // wrapped around
    assert!(r == [3, 4]);
    assert!(unsafe { c.read_ref(&mut r) }.is_err());  // not enough data
}

#[test]
fn write_ref() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(1)[0] = 0;
    let w: [u8; 2] = [1, 2];
    assert!(p.write_ref(&w).is_ok());  // not wrapped around
    c.read(1);
    let w: [u8; 2] = [3, 4];
    assert!(p.write_ref(&w).is_ok());  // wrapped around
    assert!(c.read(1)[0] == 1);
    assert!(p.write_ref(&w).is_err());  // not enough space
    assert!(c.read(1)[0] == 2);
    assert!(c.read(1)[0] == 3);
    assert!(c.read(1)[0] == 4);
}

#[test]
fn consume() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(1)[0] = 1;
    p.write(1)[0] = 2;
    p.write(1)[0] = 3;
    let mut r = c.read(usize::MAX);
    assert!(r.len() == 3);
    r.consume(1);
    assert!(r.len() == 2);
    assert!(r[0] == 2);
    assert!(r[1] == 3);
}

#[test]
#[should_panic]
fn bad_consume() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(2);
    let mut r = c.read(usize::MAX);
    r.consume(3);
}

#[test]
fn partial_drop() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, c) = b.split();
    let w = p.write(3);
    w.partial_drop(1);
    assert!(c.data_size() == 1);
}

#[test]
#[should_panic]
fn bad_partial_drop() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, _) = b.split();
    let w = p.write(2);
    w.partial_drop(3);
}

#[test]
fn zero_sized_ops() {
    let mut b: Buffer<4> = Buffer::new();
    let (mut p, mut c) = b.split();
    p.write(1);
    let mut r = c.read(0);
    r.consume(0);
    r.partial_drop(0);
    assert!(c.data_size() == 1);
    p.write(0);
    assert!(p.empty_size() == 3);
}