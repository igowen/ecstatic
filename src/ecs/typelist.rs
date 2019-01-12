// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::marker::PhantomData;

mod private {
    pub trait Sealed {}
}

pub enum Nil {}
impl private::Sealed for Nil {}

pub struct TypeCons<H, T> {
    _head: PhantomData<*const H>,
    _tail: PhantomData<*const T>,
}
impl<H, T> private::Sealed for TypeCons<H, T> {}

pub trait TypeList: private::Sealed {}

impl TypeList for Nil {}
impl<H, T> TypeList for TypeCons<H, T> {}

pub struct NotFound<T>(PhantomData<*const T>);
pub struct Found;

pub trait Append<T>: private::Sealed
where
    T: TypeList,
{
    type Output: TypeList;
}

impl<T> Append<T> for Nil
where
    T: TypeList,
{
    type Output = T;
}

impl<H, T, U> Append<U> for TypeCons<H, T>
where
    T: Append<U>,
    U: TypeList,
{
    type Output = TypeCons<H, <T as Append<U>>::Output>;
}

pub trait Consume<Target, Index>: private::Sealed {
    type Remainder: TypeList;
}

impl<Head, Tail, Target, TailIndex> Consume<Target, NotFound<TailIndex>> for TypeCons<Head, Tail>
where
    Tail: Consume<Target, TailIndex>,
{
    type Remainder = TypeCons<Head, <Tail as Consume<Target, TailIndex>>::Remainder>;
}

impl<Head, Tail: TypeList> Consume<Head, Found> for TypeCons<Head, Tail> {
    type Remainder = Tail;
}

pub trait ConsumeMultiple<Target, Indices>: private::Sealed {
    type Remainder;
}

impl<Source: private::Sealed> ConsumeMultiple<Nil, Nil> for Source {
    type Remainder = Source;
}

impl<THead, TTail, SHead, STail, IndexHead, IndexTail>
    ConsumeMultiple<TypeCons<THead, TTail>, TypeCons<IndexHead, IndexTail>>
    for TypeCons<SHead, STail>
where
    TypeCons<SHead, STail>: Consume<THead, IndexHead>,
    <TypeCons<SHead, STail> as Consume<THead, IndexHead>>::Remainder:
        ConsumeMultiple<TTail, IndexTail>,
{
    type Remainder =
        <<TypeCons<SHead, STail> as Consume<THead, IndexHead>>::Remainder as ConsumeMultiple<
            TTail,
            IndexTail,
        >>::Remainder;
}

pub trait IntoTypeList: private::Sealed {
    type Type: TypeList;
}

// TypeLists are trivially convertible to TypeLists.
impl<T> IntoTypeList for T
where
    T: TypeList,
{
    type Type = Self;
}

#[macro_export]
macro_rules! tlist {
    ($t:ty $(,)*) => { $crate::typelist::TypeCons<$t, $crate::typelist::Nil> };
    ($t:ty, $($ts:ty),+ $(,)*) => {
        $crate::typelist::TypeCons<$t, tlist!($($ts,)*)>
    };
}

// Recursive macro to implement IntoTypeList for tuples up length 32
macro_rules! impl_into_type_list {
    // Helpers for building type lists of generic types. We can't use `tlist!` because type
    // parameters don't parse as `ty`.
    (@type_cons $t:ident) => { TypeCons<$t, Nil> };
    (@type_cons $t:ident $($ts:ident)+) => {
        TypeCons<$t, impl_into_type_list!(@type_cons $($ts)*)>
    };

    (@impl_internal $($t:ident,)+) => {
        impl<$($t),*> private::Sealed for ($($t,)*) {}
        impl<$($t),*> IntoTypeList for ($($t,)*) {
            type Type = impl_into_type_list!(@type_cons $($t)*);
        }
    };

    // Base case
    (($($t:ident,)+);) => {
        impl_into_type_list!(@impl_internal $($t,)*);
    };

    // Produce the actual impl for the tuple represented by $t1, then move $t2 into the tuple and
    // recursively call impl_into_type_list
    (($($t1:ident,)+); $t2:ident $(,)* $($t3:ident),*) => {
        impl_into_type_list!(@impl_internal $($t1,)*);
        impl_into_type_list!(($($t1),*, $t2,); $($t3),*);
    };

    // Entry point
    ($t1:ident, $($t:ident),+) => {
        impl_into_type_list!(($t1,); $($t),*);
    };
}

impl_into_type_list!(
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z, AA, BB, CC, DD,
    EE, FF, GG
);
