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

//! Library for implementing the entity-component-system (ECS) pattern.
//!
//! The API is very loosely based on [`specs`](https://slide-rs.github.io/specs/), but with an
//! emphasis on statically validating the usage of the library (instead of dynamically, as specs
//! does). This comes at the cost of some flexibility, but almost all logic errors are detected at
//! compile time.
//!
//! It's also not as optimized as `specs` is (yet), since it's designed for roguelikes.
//!
//! # Usage
//!
//! Implementing an ECS requires the following:
//!
//! 1. Define the components and resources you need to store using the
//!    [`define_world!`](../macro.define_world.html) macro. This generates a struct called `World`,
//!    along with trait implementations necessary for the library to interact with it
//! 2. Implement one or more [`System`s](traits/trait.System.html)
//! 3. Run your `System`s on the World using the
//!    (`run_system`)[traits/trait.WorldInterface.html#method.run_system] method.
//!
//! # Peculiarities
//!
//! This library uses Rust's type system in a somewhat advanced manner. In the
//! [`traits`](traits/index.html) module you will find the [`Nest`](traits/trait.Nest.html) and
//! [`Flatten`](traits/trait.Flatten.html) traits, which allow flat tuples (such as `(A, B, C)`) to
//! be converted to a nested representation `(A, (B, (C, ())))` and back again. These traits are
//! implemented for tuples up to length 32, which ought to be enough for most use cases.
//!
//! Converting flat tuples to nested tuples at the API boundary allows us to implement certain
//! traits recursively, rather than needing to write macros for each trait to implement them for
//! flat tuple types. As a result, you will see type parameters that have `Nest`/`Flatten` trait
//! bounds all over the code base. Because there's no way to tell the compiler that `Nest` and
//! `Flatten` are inverse operations, occasionally you will see bounds that specify that the nested
//! represenation is also flattenable.
//!
//! Additionally, we have some [type-level metaprogramming](ecs/typelist/index.html) traits that
//! provide some amount of compile-time invariant checking.
//!
//! In general, client code shouldn't need to worry about these too much, but it does have the
//! unfortunate side effect of making compiler error messages less helpful.
//!
//! # Examples
//!
//! ```
//! # #[macro_use] extern crate ecstatic;
//! # use ecstatic::*;
//! #[derive(Debug, PartialEq)]
//! pub struct Data {
//!     x: u32,
//! }
//!
//! // `Default` impl that isn't the additive identity.
//! impl Default for Data {
//!     fn default() -> Data {
//!         Data { x: 128 }
//!     }
//! }
//!
//! #[derive(Debug, Default, PartialEq)]
//! pub struct MoreData {
//!     y: u32,
//! }
//!
//! define_world!(
//!     #[derive(Default)]
//!     pub world {
//!         components {
//!             test1: BasicVecStorage<Data>,
//!             test2: BasicVecStorage<MoreData>,
//!         }
//!         resources {}
//!     }
//! );
//!
//! let mut w = World::default();
//! w.new_entity().with(Data { x: 1 }).build();
//! w.new_entity().with(Data { x: 1 }).build();
//! let md = w
//!     .new_entity()
//!     .with(Data { x: 2 })
//!     .with(MoreData { y: 42 })
//!     .build();
//! w.new_entity().with(Data { x: 3 }).build();
//! w.new_entity().with(Data { x: 5 }).build();
//! w.new_entity().with(Data { x: 8 }).build();
//!
//! /// `TestSystem` adds up the values in every `Data` component (storing the result in `total`),
//! /// and multiplies every `MoreData` by the `Data` in the same component.
//! #[derive(Default)]
//! struct TestSystem {
//!     total: u32,
//! }
//!
//! impl<'a> System<'a> for TestSystem {
//!     type Dependencies = (
//!         ReadComponent<'a, Data>,
//!         WriteComponent<'a, MoreData>,
//!     );
//!     fn run(&'a mut self, (data, mut more_data): Self::Dependencies) {
//!         self.total = 0;
//!
//!         (&data,).for_each(|_, (d,)| {
//!             self.total += d.x;
//!         });
//!
//!         (&data, &mut more_data).for_each(|_, (d, md)| {
//!             md.y *= d.x;
//!         });
//!     }
//! }
//!
//! let mut system = TestSystem::default();
//! w.run_system(&mut system);
//!
//! assert_eq!(system.total, 20);
//! assert_eq!(
//!     <World as GetComponent<'_, MoreData>>::get(&w).get(md),
//!     Some(&MoreData { y: 84 })
//! );
//! ```
//!
//! Components accessed via `ReadComponent` cannot be iterated over mutably:
//!
//! ```compile_fail
//! # #[macro_use] extern crate ecstatic;
//! # use ecstatic::*;
//! #[derive(Debug, PartialEq)]
//! pub struct Data {
//!     x: u32,
//! }
//!
//! define_world!(
//!     pub world {
//!         components {
//!             test1: BasicVecStorage<Data>,
//!         }
//!         resources {}
//!     }
//! );
//!
//! #[derive(Default)]
//! struct TestSystem {}
//!
//! impl<'a> System<'a> for TestSystem {
//!     type Dependencies = (
//!         ReadComponent<'a, Data>,
//!     );
//!     fn run(&'a mut self, (data,): Self::Dependencies) {
//!         (&mut data,).for_each(|(d,)| {
//!             // do something
//!         });
//!     }
//! }
//! ```
//!
#[macro_use]
pub mod typelist;

/// Traits used in the ECS interface(s)
pub mod traits;

/// Component storage infrastructure
pub mod storage;

pub mod join;

mod bitset;

pub use crate::join::*;
pub use crate::storage::*;
pub use crate::traits::*;

/// `Entity` is an opaque identifier that can be used to look up associated components in a
/// `World`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Entity {
    /// The id of this entity within the world.
    pub id: usize,
    /// The generation of this entity.
    pub generation: usize,
}

/// Defines the set of data structures necessary for using `ecstatic`.
///
/// Generates the following structs:
/// - `Resources`
///   - All of the components and resources
/// - `World`
///   - Wraps `Resources` and contains entity metadata
/// - `EntityBuilder`
///   - Helper for `World::new_entity()`
/// - `ComponentSet`
///   - Used by `EntityBuilder`. Basically just all of the components wrapped in an `Option`.
///
/// # Example
/// ```
/// # #[macro_use] extern crate ecstatic;
/// # use ecstatic::*;
/// #[derive(Default, Debug)]
/// struct Data {
///     info: String,
/// }
///
/// define_world!(
///     // You can apply trait derivations to the output structs. Whatever is specified here will
///     // apply to both the `World` struct and the `Resources` struct.
///     #[derive(Default, Debug)]
///     // The visibility specifier is optional. It applies to all of the types defined by the
///     // macro.
///     pub(crate) world {
///         // Components must all go in collections that implement `ComponentStorage`. They are
///         // addressed by type, so you can only have one field per type.
///         components {
///             strings: BasicVecStorage<Data>,
///         }
///         // Resources are just stored bare, but the same restriction on unique fields per type
///         // applies (but only within resources -- you can have a resource of the same type as a
///         // component).
///         resources {
///             data: Data,
///         }
///     }
/// );
/// ```
#[macro_export(local_inner_macros)]
macro_rules! define_world {
    ($(#[$meta:meta])*
     $v:vis world {
        components {
            $($component:ident : $($component_storage:ident) :: + < $component_type:ty >),* $(,)*
        }
        resources {
            $($resource:ident : $resource_type:ty),* $(,)*
        }
    }) => {
        __define_world_internal!{@impl_storage_spec {$($component_type; $($component_storage)::*)*}}
        __define_world_internal!{@impl_get_component $({$component $component_type})*}
        __define_world_internal!{@impl_get_resource $({$resource $resource_type})*}
        __define_world_internal!{@define_world_struct
            $(#[$meta])* $v ($($component: $component_type)*)}
        __define_world_internal!{@define_builder_struct $v $($component:$component_type)*}
        $(
            __define_world_internal!{@impl_build_with $component $component_type}
        )*
        __define_world_internal!{@define_resource_struct $(#[$meta])* $v
            (
                {$($component:($($component_storage)::*; $component_type))*}
                {$($resource : $resource_type)*}
            )
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __define_world_internal {
    (@impl_storage_spec {$($component_type:ty; $($component_storage:ident)::+ )*}) => {
        $(
            impl<'a> $crate::StorageSpec<'a> for $component_type {
                type Storage = $($component_storage)::* <$component_type>;
                type Component = $component_type;
            }
        )*
    };

    (@impl_get_resource $({$resource:ident $resource_type:ty})*) => {
        $(
            impl GetResource<$resource_type> for World {
                fn get(&self) -> std::cell::Ref<$resource_type> {
                    self.resources.$resource.borrow()
                }
                fn get_mut(&self) -> std::cell::RefMut<$resource_type> {
                    self.resources.$resource.borrow_mut()
                }
                fn set(&self, t: $resource_type) {
                    self.resources.$resource.replace(t);
                }
            }
        )*
    };

    (@impl_get_component $({$component:ident $component_type:ty})*) => {
        $(
            impl<'a> GetComponent<'a, $component_type> for World {
                fn get(&self) -> std::cell::Ref<<$component_type as StorageSpec<'a>>::Storage> {
                    self.resources.$component.borrow()
                }
                fn get_mut(&self) -> std::cell::RefMut<<$component_type as StorageSpec<'a>>::Storage> {
                    self.resources.$component.borrow_mut()
                }
            }
        )*
    };

    (@define_resource_struct $(#[$meta:meta])* $v:vis (
                             {$($component:ident : ($($component_storage:ident) :: +; $component_type:ty))*}
                             {$($resource:ident : $resource_type:ty)*})) => {
        $(#[$meta])*
        $v struct Resources {
            $(
                $component: std::cell::RefCell<$($component_storage)::*<$component_type>>,
            )*

            $(
                $resource: std::cell::RefCell<$resource_type>,
            )*
        }
    };

    (@define_world_struct $(#[$meta:meta])* $v:vis
                          ($($component:ident : $type:ty)*)) => {
        /// Encapsulation of a set of component and resource types. Also provides a means for
        /// constructing new entities.
        $(#[$meta])*
        $v struct World {
            resources: Resources,
            num_entities: usize,
            free_list: Vec<Entity>,
        }

        impl $crate::ResourceProvider for World {
            type Resources = Resources;
            fn get_resources(&mut self) -> &Self::Resources {
                &self.resources
            }
        }

        impl<'a> $crate::WorldInterface<'a> for World {
            type EntityBuilder = EntityBuilder<'a>;
            type ComponentSet = ComponentSet;
            type AvailableTypes = tlist!($($type),*);

            fn new_entity(&'a mut self) -> Self::EntityBuilder {
                EntityBuilder {
                    components: ComponentSet{
                    $(
                        $component: None,
                    )*
                    },
                    world: self,
                }
            }

            fn build_entity(&mut self, components: Self::ComponentSet) -> Entity {
                use $crate::ComponentStorage;
                let mut entity;
                if let Some(e) = self.free_list.pop() {
                    entity = e;
                    entity.generation += 1;
                } else {
                    entity = Entity{
                        id:self.num_entities,
                        generation: 0,
                    };
                    self.num_entities += 1;
                }
                $(
                    // Should never panic, since having a mutable reference to `self` implies that
                    // there are no extant immutable references.
                    self.resources.$component.borrow_mut().set(entity, components.$component);
                )*
                entity
            }

            fn delete_entity(&mut self, entity: Entity) {
                use $crate::ComponentStorage;
                if entity.id < self.num_entities {
                    $(
                        self.resources.$component.borrow_mut().set(entity, None);
                    )*
                    self.free_list.push(entity);
                }
            }
        }
    };

    (@define_builder_struct $v:vis $($field:ident:$type:ty)*) => {
        #[derive(Default)]
        /// ComponentSet is roughly equivalent to a tuple containing Option<T> for all types the
        /// World stores.
        $v struct ComponentSet {
            $(
                $field: Option<$type>,
            )*
        }
        /// Builder pattern for creating new entities.
        $v struct EntityBuilder<'a> {
            components: ComponentSet,
            world: &'a mut World,
        }
        impl<'a> EntityBuilder<'a> {
            /// Finalize this entity and all of its components by storing them in the `World`.
            $v fn build(self) -> Entity {
                use $crate::WorldInterface;
                self.world.build_entity(self.components)
            }
        }
    };

    (@impl_build_with $field:ident $type:ty) => {
        impl<'a> $crate::BuildWith<$type> for EntityBuilder<'a> {
            fn with(mut self, data: $type) -> Self {
                self.components.$field = Some(data);
                self
            }
        }
    };
}

// Need to put this down here because the macro definitions have to come first :/
#[cfg(test)]
mod tests;
