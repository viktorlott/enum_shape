pub trait Shl<Rhs = Self> {
    type Output;

    fn shl(self, rhs: Rhs) -> Self::Output;
}
