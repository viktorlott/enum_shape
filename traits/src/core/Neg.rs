pub trait Neg {
    type Output;

    fn neg(self) -> Self::Output;
}
