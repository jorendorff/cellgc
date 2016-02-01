/// The `gc_heap_type!` macro can declare structs and enums for use with `Heap::alloc`.
///
/// The argument to `gc_heap_type! is a struct or enum, with the following syntax:
///
/// ```ignore
/// heap-type:
///     attr* heap-struct
///     attr* heap-enum
///
/// attr:
///     "#" "[" META "]"
///
/// heap-struct:
///     "pub"? "struct" IDENT "/" IDENT "/" IDENT "<'h>" "{" heap-struct-field,* "}"
///
/// heap-struct-field:
///     IDENT / IDENT ":" TYPE
///
/// heap-enum:
///     "pub"? "enum" IDENT "/" IDENT "<'h>" "{" heap-enum-variant,* "}"
///
/// heap-enum-variant:
///     IDENT
///     IDENT "(" TYPE,* ")"
/// ```
///
/// This syntax is almost a subset of real Rust struct and enum syntax. The only
/// difference is that in some places we require two or three identifiers,
/// instead of one:
///
/// *   The three names of a struct are: (1) the type you'll use to create instances
///     and call `heap.alloc` with; (2) a `Ref` smart pointer type (what `heap.alloc`
///     returns; and (3) the in-heap version of the struct, which you can just ignore.
///     Attributes that appear in your code are applied only to the first of these
///     three types.
///
/// *   The two names of a struct field are:  (1) the field name, which doubles as
///     the name of the getter on the `Ref` struct; (2) the setter name.
///
/// *   The two names of an enum are (1) the type you'll use; (2) the in-heap
///     version of the enum, which you can just ignore. Threre's not a `Ref`
///     type because at the moment, we don't support *direct* allocation of
///     enums in the heap; they can only be fields of heap structs.
///
/// The exact lifetime name `'h` is required. (A bizarre restriction - but
/// I had little success getting the macro to accept an arbitrary lifetime
/// passed in by the macro caller.)
///
/// Trailing commas are not supported everywhere they should be. (Sorry!)
///
/// # Examples
///
/// A very simple "object" type for a text adventure game:
///
/// ```rust
/// #[macro_use] extern crate cell_gc;
/// use cell_gc::collections::VecRef;
///
/// gc_heap_type! {
///     struct Object / ObjectRef / ObjectInHeap <'h> {
///         name / set_name : String,
///         description / set_description: String,
///         children / set_children: VecRef<'h, ObjectRef<'h>>
///     }
/// }
/// # fn main() {}
/// ```
///
/// Note that `children` is a `VecRef<'h, ObjectRef<'h>>`; that is, it is
/// a reference to a separately GC-allocated `Vec<ObjectRef<'h>>`, which is
/// a vector of references to other objects. In other words, this is exactly
/// what you would have in Java for a field declared like this:
///
/// ```java
/// public ArrayList<Object> children;
/// ```
///
/// The API generated by this macro looks like this:
///
/// ```rust
/// # struct VecRef<'h, T: 'h>(&'h T);  // hack to make this compile
/// struct Object<'h> {
///     name: String,
///     description: String,
///     children: VecRef<'h, ObjectRef<'h>>
/// }
///
/// struct ObjectRef<'h> {
///    /* all fields private */
/// #  target: &'h Object<'h>     // hack to make this compile
/// }
///
/// impl<'h> ObjectRef<'h> {
///     fn name(&self) -> String
/// #       { unimplemented!(); }
///     fn set_name(&self, name: String)
/// #       { unimplemented!(); }
///     fn description(&self) -> String
/// #       { unimplemented!(); }
///     fn set_description(&self, description: String)
/// #       { unimplemented!(); }
///     fn children(&self) -> VecRef<'h, ObjectRef<'h>>
/// #       { unimplemented!(); }
///     fn set_children(&self, children: VecRef<'h, ObjectRef<'h>>)
/// #       { unimplemented!(); }
/// }
/// ```
///
/// (You may never actually use that `set_children()` method.
/// Instead, you'll initialize the `children` field with a vector when you
/// create the object, and then you'll most likely mutate that existing vector
/// rather than ever creating a new one.)
///
/// You can allocate `Object`s in the heap using `heap.alloc(Object { ... })`,
/// and make one `Object` a child of another by using `obj1.children().push(obj2)`.
///
#[macro_export]
macro_rules! gc_heap_type {
    // Top-level rules.
    { $(#[$attr:meta])* pub enum $($etc:tt)* } =>
    { gc_heap_type! { @gc_heap_enum ($(#[$attr])*) (pub) enum $($etc)* } };

    { $(#[$attr:meta])* enum $($etc:tt)* } =>
    { gc_heap_type! { @gc_heap_enum ($(#[$attr])*) () enum $($etc)* } };

    { $(#[$attr:meta])* pub struct $($etc:tt)* } =>
    { gc_heap_type! { @gc_heap_struct ($(#[$attr])*) (pub) struct $($etc)* } };

    { $(#[$attr:meta])* struct $($etc:tt)* } =>
    { gc_heap_type! { @gc_heap_struct ($(#[$attr])*) () struct $($etc)* } };

    // Helpers used by almost every macro.
    { @as_item $x:item } => { $x };
    { @as_expr $x:expr } => { $x };

    // The main helper macro for expanding a struct.
    {
        @gc_heap_struct ( $(#[$attr:meta])* ) ( $($maybe_pub:tt)* )
        struct $fields_type:ident / $ref_type:ident / $storage_type:ident <'h> {
            $($field_name:ident / $field_setter_name:ident : $field_type: ty),*
        }
    } => {
        // === $storage_type: the in-heap representation of the struct
        gc_heap_type! {
            @as_item
            $($maybe_pub)* struct $storage_type<'h> {
                $( pub $field_name: <$field_type as $crate::traits::IntoHeap<'h>>::In ),*
            }
        }

        // === $fields_type: A safe version of the struct
        gc_heap_type! {
            @as_item
            $(#[$attr])*
            $($maybe_pub)* struct $fields_type<'h> {
                $( pub $field_name: $field_type ),*
            }
        }

        unsafe impl<'h> $crate::traits::IntoHeap<'h> for $fields_type<'h> {
            type In = $storage_type<'h>;

            fn into_heap(self) -> $storage_type<'h> {
                $storage_type {
                    $( $field_name: $crate::traits::IntoHeap::into_heap(self.$field_name) ),*
                }
            }

            unsafe fn mark(storage: &$storage_type<'h>) {
                if !$crate::Heap::get_mark_bit::<Self>(storage) {
                    $crate::Heap::set_mark_bit::<Self>(storage);
                    $(
                        <$field_type as $crate::traits::IntoHeap>::mark(&storage.$field_name);
                    )*
                }
            }

            unsafe fn from_heap(storage: &$storage_type<'h>) -> $fields_type<'h> {
                $fields_type {
                    $( $field_name: $crate::traits::IntoHeap::from_heap(&storage.$field_name) ),*
                }
            }
        }

        impl<'h> $crate::traits::IntoHeapAllocation<'h> for $fields_type<'h> {
            type Ref = $ref_type<'h>;

            fn wrap_gcref(gcref: $crate::GCRef<'h, $fields_type<'h>>) -> $ref_type<'h> {
                $ref_type(gcref)
            }
        }

        // === $ref_type: A safe reference to the struct
        gc_heap_type! {
            @as_item
            #[derive(Clone, Debug, PartialEq, Eq)]
            $($maybe_pub)* struct $ref_type<'h>($crate::GCRef<'h, $fields_type<'h>>);
        }

        unsafe impl<'h> $crate::traits::IntoHeap<'h> for $ref_type<'h> {
            type In = *mut $storage_type<'h>;

            fn into_heap(self) -> *mut $storage_type<'h> {
                self.0.as_mut_ptr()
            }

            unsafe fn from_heap(storage: &*mut $storage_type<'h>) -> $ref_type<'h> {
                $ref_type($crate::GCRef::new(*storage))
            }

            unsafe fn mark(storage: &*mut $storage_type<'h>) {
                let ptr = *storage;
                if !ptr.is_null() {
                    <$fields_type<'h> as $crate::traits::IntoHeap>::mark(&*ptr);
                }
            }
        }

        impl<'h> $ref_type<'h> {
            // Field accessors.
            $(
                pub fn $field_name(&self) -> $field_type {
                    let ptr = self.0.as_ptr();
                    unsafe {
                        $crate::traits::IntoHeap::from_heap(&(*ptr).$field_name)
                    }
                }

                pub fn $field_setter_name(&self, v: $field_type) {
                    let ptr = self.0.as_mut_ptr();
                    let u = $crate::traits::IntoHeap::into_heap(v);
                    unsafe {
                        (*ptr).$field_name = u;
                    }
                }
            )*

            pub fn as_mut_ptr(&self) -> *mut $storage_type<'h> {
                self.0.as_mut_ptr()
            }
        }
    };

    // `gc_heap_type! { @for_each_variant ($helper*) {$variants*} {} ($ctn*) }`
    //
    // This helper is like `concatMap` for mapping enum variants through
    // another helper macro. `{$variants*}` should be the body of an enum item.
    //
    // For each variant in $variants, this calls
    // `gc_heap_type! { $helper $variant_name $variant_fields ($ctn2*) }`.
    // For variants that have no fields, it passes `NO_FIELDS` to the
    // $variant_fields argument.  The helper must call back:
    // `gc_heap_type! { $ctn2* { ...tokens... } }`.
    //
    // @for_each_variant accumulates all the tokens passed back by the calls to
    // $helper. After the last call to $helper, it passes all the results to
    // its continuation, calling `gc_heap_type! { $ctn* { ...all tokens... } }`.
    {
        @for_each_variant $_helper:tt {} $all_results:tt ($($ctn:tt)*)
    } => {
        gc_heap_type! { $($ctn)* $all_results }
    };

    {
        @for_each_variant ($($helper:tt)*)
        { $variant_name:ident } $acc:tt $ctn:tt
    } => {
        gc_heap_type! {
            $($helper)* $variant_name NO_FIELDS
                (@next_variant ($($helper)*) {} $acc $ctn)
        }
    };

    {
        @for_each_variant ($($helper:tt)*)
        { $variant_name:ident , $($more_variants:tt)* }
        $acc:tt $ctn:tt
    } => {
        gc_heap_type! {
            $($helper)* $variant_name NO_FIELDS
                (@next_variant ($($helper)*) { $($more_variants)* } $acc $ctn)
        }
    };

    {
        @for_each_variant ($($helper:tt)*)
        { $variant_name:ident ( $($field_types:tt)* ) }
        $acc:tt $ctn:tt
    } => {
        gc_heap_type! {
            $($helper)* $variant_name ( $($field_types)* )
                (@next_variant ($($helper)*) {} $acc $ctn)
        }
    };

    {
        @for_each_variant ($($helper:tt)*)
        { $variant_name:ident ( $($field_types:tt)* ), $($more_variants:tt)*  }
        $acc:tt $ctn:tt
    } => {
        gc_heap_type! {
            $($helper)* $variant_name ( $($field_types)* )
                (@next_variant ($($helper)*) { $($more_variants)* } $acc $ctn)
        }
    };

    {
        @for_each_variant ($($helper:tt)*)
        { $variant_name:ident { $($field_name:ident : $field_type:ty),* } }
        $acc:tt $ctn:tt
    } => {
        gc_heap_type! {
            $($helper)* $variant_name { $($field_name : $field_type),* }
                (@next_variant ($($helper)*) {} $acc $ctn)
        }
    };

    {
        @for_each_variant ($($helper:tt)*)
        { $variant_name:ident { $($field_name:ident : $field_type:ty),* }, $($more_variants:tt)*  }
        $acc:tt $ctn:tt
    } => {
        gc_heap_type! {
            $($helper)* $variant_name { $($field_name : $field_type),* }
                (@next_variant ($($helper)*) { $($more_variants)* } $acc $ctn)
        }
    };

    {
        @next_variant $helper:tt $more_variants:tt { $($acc:tt)* } $ctn:tt { $($rv:tt)* }
    } => {
        gc_heap_type! {
            @for_each_variant $helper $more_variants { $($acc)* $($rv)* } $ctn
        }
    };

    // `gc_heap_type! { @zip_idents_with_types ($alphabet*) ($parens_types*) () ($ctn*) }`
    //
    // This helper macro pairs each parenthesized type in `$parens_types*`
    // with a letter of the `$alphabet*`. It passes the resulting pairs
    // to `$ctn`. So, for example, this:
    //     @zip_idents_with_types (a b c d e f g) ((i32) (String)) () (@continue_here)
    // boils down to this:
    //     @continue_here ((a: i32) (b: String))
    {
        @zip_idents_with_types $_leftovers:tt () ($(($binding:ident : $btype:ty))*) ($($ctn:tt)*)
    } => {
        gc_heap_type! { $($ctn)* ($(($binding : $btype))*) }
    };
    {
        @zip_idents_with_types
        ($id:ident $($ids:tt)*)
        (($t:ty) $($ts:tt)*)
        ($($acc:tt)*)
        $ctn:tt
    } => {
        gc_heap_type! {
            @zip_idents_with_types
            ($($ids)*)
            ($($ts)*)
            ($($acc)* ($id : $t))
            $ctn
        }
    };

    // Helper rules for declaring an in-heap enum.
    {
        @enum_in_heap_variant $variant_name:ident NO_FIELDS ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* { $variant_name, }
        }
    };

    {
        @enum_in_heap_variant $variant_name:ident ( $($field_type:ty),* ) ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* {
                $variant_name($(<$field_type as $crate::traits::IntoHeap<'h>>::In),*),
            }
        }
    };

    {
        @enum_in_heap_variant $variant_name:ident { $($field_name:ident : $field_type:ty),* } ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* {
                $variant_name { $($field_name : <$field_type as $crate::traits::IntoHeap<'h>>::In),* },
            }
        }
    };

    {
        @enum_declare_in_heap_type ( $($maybe_pub:tt)* ) $storage_type:ident
            { $($variants:tt)* }
    } => {
        gc_heap_type! {
            @as_item
            $($maybe_pub)*
            enum $storage_type<'h> {
                $($variants)*
            }
        }
    };

    // Helper rules for implementing the mark() method for an in-heap enum.
    {
        @enum_mark_variant $storage_type:ident
            $name:ident NO_FIELDS ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* { $storage_type::$name => (), }
        }
    };

    {
        @enum_mark_variant $storage_type:ident
            $name:ident ( $($field_type:ty),* ) $ctn:tt
    } => {
        gc_heap_type! {
            @zip_idents_with_types (a b c d e f g h i j k l m n o p q r s t u v w x y z)
                ( $( ($field_type) )* ) ()
                (@enum_mark_variant_continued $storage_type $name $ctn)
        }
    };

    {
        @enum_mark_variant_continued $storage_type:ident $name:ident ($($ctn:tt)*)
            ( $(($binding:ident : $field_type:ty))* )
    } => {
        gc_heap_type! {
            $($ctn)* {
                $storage_type::$name ( $(ref $binding),* ) => {
                    $( <$field_type as $crate::traits::IntoHeap>::mark($binding); )*
                },
            }
        }
    };

    {
        @enum_mark_variant $storage_type:ident
            $name:ident { $($field_name:ident : $field_type:ty),* } ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* {
                $storage_type::$name { $(ref $field_name),* } => {
                    $( <$field_type as $crate::traits::IntoHeap>::mark($field_name); )*
                },
            }
        }
    };

    {
        @enum_mark_expr ($self_ref:expr) { $($arms:tt)* }
    } => {
        gc_heap_type! {
            @as_expr
            match *$self_ref {
                $($arms)*
            }
        }
    };

    // Helper rules for implementing the into_heap() method for an IntoHeap
    // enum.
    {
        @enum_into_heap_variant $stack_type:ident $storage_type:ident
            $name:ident NO_FIELDS
            ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* { $stack_type::$name => $storage_type::$name, }
        }
    };

    {
        @enum_into_heap_variant $stack_type:ident $storage_type:ident
            $name:ident ( $($field_type:ty),* )
            $ctn:tt
    } => {
        gc_heap_type! {
            @zip_idents_with_types (a b c d e f g h i j k l m n o p q r s t u v w x y z)
                ( $( ($field_type) )* ) ()
                (@enum_into_heap_variant_continued $stack_type $storage_type $name $ctn)
        }
    };

    {
        @enum_into_heap_variant_continued $stack_type:ident $storage_type:ident $name:ident ($($ctn:tt)*)
            ( $(($binding:ident : $field_type:ty))* )
    } => {
        gc_heap_type! {
            $($ctn)* {
                $stack_type::$name ( $($binding),* ) =>
                    $storage_type::$name( $($crate::traits::IntoHeap::into_heap($binding)),* ),
            }
        }
    };

    {
        @enum_into_heap_variant $stack_type:ident $storage_type:ident
            $name:ident { $($field_name:ident : $field_type:ty),* }
            ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* {
                $stack_type::$name { $($field_name),* } => $storage_type::$name {
                    $( $field_name : $crate::traits::IntoHeap::into_heap($field_name) ),*
                },
            }
        }
    };

    {
        @enum_into_heap_expr ($self_:expr)
        { $($accumulated_output:tt)* }
    } => {
        gc_heap_type! {
            @as_expr
            match $self_ {
                $($accumulated_output)*
            }
        }
    };

    // Helper rules for implementing the from_heap() method of an in-heap enum.
    {
        @enum_from_heap_variant $stack_type:ident $storage_type:ident
            $name:ident NO_FIELDS ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* { &$storage_type::$name => $stack_type::$name, }
        }
    };

    {
        @enum_from_heap_variant $stack_type:ident $storage_type:ident
            $name:ident ( $($field_type:ty),* ) $ctn:tt
    } => {
        gc_heap_type! {
            @zip_idents_with_types (a b c d e f g h i j k l m n o p q r s t u v w x y z)
                ( $( ($field_type) )* ) ()
                (@enum_from_heap_variant_continued $stack_type $storage_type $name $ctn)
        }
    };

    {
        @enum_from_heap_variant_continued $stack_type:ident $storage_type:ident $name:ident ($($ctn:tt)*)
            ( $(($binding:ident : $field_type:ty))* )
    } => {
        gc_heap_type! {
            $($ctn)* {
                &$storage_type::$name ( $(ref $binding),* ) =>
                    $stack_type::$name( $($crate::traits::IntoHeap::from_heap($binding)),* ),
            }
        }
    };

    {
        @enum_from_heap_variant $stack_type:ident $storage_type:ident
            $name:ident { $($field_name:ident : $field_type:ty),* } ($($ctn:tt)*)
    } => {
        gc_heap_type! {
            $($ctn)* {
                &$storage_type::$name { $(ref $field_name),* } => $stack_type::$name {
                    $( $field_name: $crate::traits::IntoHeap::from_heap($field_name) ),*
                },
            }
        }
    };

    {
        @enum_from_heap_expr ($self_ref:expr) { $($arms:tt)* }
    } => {
        gc_heap_type! {
            @as_expr
            match $self_ref {
                $($arms)*
            }
        }
    };

    {
        @gc_heap_enum
        ($(#[$attr:meta])*)
        ($($maybe_pub:tt)*)
        enum $stack_type:ident / $storage_type:ident <'h>
        $variants:tt
    } => {
        gc_heap_type! {
            @for_each_variant (@enum_in_heap_variant) $variants {}
                (@enum_declare_in_heap_type ( $($maybe_pub)* ) $storage_type)
        }

        gc_heap_type! {
            @as_item
            $(#[$attr])*
            $($maybe_pub)* enum $stack_type<'h>
                $variants
        }

        unsafe impl<'h> $crate::traits::IntoHeap<'h> for $stack_type<'h> {
            type In = $storage_type<'h>;

            fn into_heap(self) -> $storage_type<'h> {
                gc_heap_type! {
                    @for_each_variant (@enum_into_heap_variant $stack_type $storage_type) $variants {}
                    (@enum_into_heap_expr (self))
                }
            }

            unsafe fn from_heap(storage: &$storage_type<'h>) -> $stack_type<'h> {
                gc_heap_type! {
                    @for_each_variant (@enum_from_heap_variant $stack_type $storage_type) $variants {}
                    (@enum_from_heap_expr (storage))
                }
            }

            unsafe fn mark(storage: &$storage_type<'h>) {
                gc_heap_type! {
                    @for_each_variant (@enum_mark_variant $storage_type) $variants {}
                    (@enum_mark_expr (storage))
                }
            }
        }
    }
}
