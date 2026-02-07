use test_stubs::test_stubs;

// Empty / non-recursive types
#[test_stubs]
trait SimpleT {
    fn x(&self);
    fn y(&self, x: u8) -> u8;
}

#[test]
fn simplet() {
    struct S;
    impl SimpleT for S {
        fn y(&self, x: u8) -> u8 {
            x
        }
    }

    let s = S;
    let _ = std::panic::catch_unwind(|| s.x());
    assert_eq!(s.y(8), 8);
}

#[test]
#[should_panic(expected = "not yet implemented: x")]
fn name_of_fn() {
    struct S;
    impl SimpleT for S {}

    S.x();
}

// `impl Iterator`
#[test_stubs]
trait IterT {
    fn iter(&self) -> impl Iterator<Item = u8>;
    fn iter2(&self) -> impl Iterator<Item = u8>;
    fn opt_iter(&self) -> Option<impl Iterator<Item = u8>>;
    fn opt_iter2(&self) -> Option<impl Iterator<Item = u8>>;
}

#[test]
fn itert() {
    struct S;
    impl IterT for S {
        fn iter2(&self) -> impl Iterator<Item = u8> {
            [2].into_iter()
        }

        fn opt_iter2(&self) -> Option<impl Iterator<Item = u8>> {
            Some([2].into_iter())
        }
    }

    let s = S;
    let _ = std::panic::catch_unwind(|| s.iter());
    assert_eq!(s.iter2().collect::<Vec<_>>().as_slice(), &[2]);
    let _ = std::panic::catch_unwind(|| s.opt_iter());
    assert_eq!(s.opt_iter2().unwrap().collect::<Vec<_>>().as_slice(), &[2]);
}

// Tuples
#[test_stubs]
trait TupleT {
    fn x(&self) -> (u8, u8);
    fn x2(&self) -> (u8, u8);
}

#[test]
fn nested_runtime_call() {
    struct S;
    impl TupleT for S {
        fn x2(&self) -> (u8, u8) {
            (1, 2)
        }
    }

    let s = S;
    let _ = std::panic::catch_unwind(|| s.x());
    assert_eq!(s.x2(), (1, 2));
}

// `self` types
#[test_stubs]
trait SelfT {
    fn x(self) -> u8;
    fn x2(self) -> u8;
}

#[test]
fn selft() {
    struct S;
    impl SelfT for S {
        fn x2(self) -> u8 {
            3
        }
    }

    let _ = std::panic::catch_unwind(|| S.x());
    assert_eq!(S.x2(), 3);
}
