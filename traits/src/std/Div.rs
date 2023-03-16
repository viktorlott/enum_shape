pub trait Div<Rhs = Self> {
    type Output;

    fn div(self, rhs: Rhs) -> Self::Output;
}
