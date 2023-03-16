pub trait Rem<Rhs = Self> {
    type Output;

    fn rem(self, rhs: Rhs) -> Self::Output;
}
