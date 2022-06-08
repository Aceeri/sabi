pub mod attr;
pub mod ctxt;
pub mod derive;
pub mod respan;
pub mod symbol;

pub use ctxt::Ctxt;
pub use derive::derive;

use syn::Type;

pub fn ungroup(mut ty: &Type) -> &Type {
    while let Type::Group(group) = ty {
        ty = &group.elem;
    }
    ty
}
