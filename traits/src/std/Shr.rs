pub trait Shr<Rhs = Self> {
    type Output;

    fn shr(self, rhs: Rhs) -> Self::Output;
}
