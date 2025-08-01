use either::{
	Either,
};

#[derive(Clone, Copy, Debug)]
pub enum OneOrMany<T>
where
	T: 'static + Copy,
{
	One(T),
	Many(&'static [T]),
}

impl<T> OneOrMany<T>
where
	T: 'static + Copy,
{
	pub fn is_one(self) -> bool {
		matches!(self, Self::One(_))
	}
}

impl<T> IntoIterator for OneOrMany<T>
where
	T: Copy,
{
	type Item = T;
	type IntoIter = Either<std::iter::Once<T>, std::iter::Copied<std::slice::Iter<'static, T>>>;
	
	fn into_iter(self) -> Self::IntoIter {
		match self {
			Self::One(item) => Either::Left(std::iter::once(item)),
			Self::Many(items) => Either::Right(items.iter().copied()),
		}
	}
}
