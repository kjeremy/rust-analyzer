use ra_ide_db::RootDatabase;
use ra_syntax::{
    ast::{self, AstNode, NameOwner},
    TextSize,
};
use stdx::format_to;

use crate::{utils::FamousDefs, Assist, AssistCtx, AssistId};
use test_utils::tested_by;

// Assist add_from_impl_for_enum
//
// Adds a From impl for an enum variant with one tuple field
//
// ```
// enum A { <|>One(u32) }
// ```
// ->
// ```
// enum A { One(u32) }
//
// impl From<u32> for A {
//     fn from(v: u32) -> Self {
//         A::One(v)
//     }
// }
// ```
pub(crate) fn add_from_impl_for_enum(ctx: AssistCtx) -> Option<Assist> {
    let variant = ctx.find_node_at_offset::<ast::EnumVariant>()?;
    let variant_name = variant.name()?;
    let enum_name = variant.parent_enum().name()?;
    let field_list = match variant.kind() {
        ast::StructKind::Tuple(field_list) => field_list,
        _ => return None,
    };
    if field_list.fields().count() != 1 {
        return None;
    }
    let field_type = field_list.fields().next()?.type_ref()?;
    let path = match field_type {
        ast::TypeRef::PathType(p) => p,
        _ => return None,
    };

    if existing_from_impl(ctx.sema, &variant).is_some() {
        tested_by!(test_add_from_impl_already_exists);
        return None;
    }

    ctx.add_assist(
        AssistId("add_from_impl_for_enum"),
        "Add From impl for this enum variant",
        |edit| {
            let start_offset = variant.parent_enum().syntax().text_range().end();
            let mut buf = String::new();
            format_to!(
                buf,
                r#"

impl From<{0}> for {1} {{
    fn from(v: {0}) -> Self {{
        {1}::{2}(v)
    }}
}}"#,
                path.syntax(),
                enum_name,
                variant_name
            );
            edit.insert(start_offset, buf);
            edit.set_cursor(start_offset + TextSize::of("\n\n"));
        },
    )
}

fn existing_from_impl(
    sema: &'_ hir::Semantics<'_, RootDatabase>,
    variant: &ast::EnumVariant,
) -> Option<()> {
    let variant = sema.to_def(variant)?;
    let enum_ = variant.parent_enum(sema.db);
    let krate = enum_.module(sema.db).krate();

    let from_trait = FamousDefs(sema, krate).core_convert_From()?;

    let enum_type = enum_.ty(sema.db);

    let wrapped_type = variant.fields(sema.db).get(0)?.signature_ty(sema.db);

    if enum_type.impls_trait(sema.db, from_trait, &[wrapped_type]) {
        Some(())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tests::{check_assist, check_assist_not_applicable};
    use test_utils::covers;

    #[test]
    fn test_add_from_impl_for_enum() {
        check_assist(
            add_from_impl_for_enum,
            "enum A { <|>One(u32) }",
            r#"enum A { One(u32) }

<|>impl From<u32> for A {
    fn from(v: u32) -> Self {
        A::One(v)
    }
}"#,
        );
    }

    #[test]
    fn test_add_from_impl_for_enum_complicated_path() {
        check_assist(
            add_from_impl_for_enum,
            "enum A { <|>One(foo::bar::baz::Boo) }",
            r#"enum A { One(foo::bar::baz::Boo) }

<|>impl From<foo::bar::baz::Boo> for A {
    fn from(v: foo::bar::baz::Boo) -> Self {
        A::One(v)
    }
}"#,
        );
    }

    fn check_not_applicable(ra_fixture: &str) {
        let fixture =
            format!("//- main.rs crate:main deps:core\n{}\n{}", ra_fixture, FamousDefs::FIXTURE);
        check_assist_not_applicable(add_from_impl_for_enum, &fixture)
    }

    #[test]
    fn test_add_from_impl_no_element() {
        check_not_applicable("enum A { <|>One }");
    }

    #[test]
    fn test_add_from_impl_more_than_one_element_in_tuple() {
        check_not_applicable("enum A { <|>One(u32, String) }");
    }

    #[test]
    fn test_add_from_impl_struct_variant() {
        check_not_applicable("enum A { <|>One { x: u32 } }");
    }

    #[test]
    fn test_add_from_impl_already_exists() {
        covers!(test_add_from_impl_already_exists);
        check_not_applicable(
            r#"
enum A { <|>One(u32), }

impl From<u32> for A {
    fn from(v: u32) -> Self {
        A::One(v)
    }
}
"#,
        );
    }

    #[test]
    fn test_add_from_impl_different_variant_impl_exists() {
        check_assist(
            add_from_impl_for_enum,
            r#"enum A { <|>One(u32), Two(String), }

impl From<String> for A {
    fn from(v: String) -> Self {
        A::Two(v)
    }
}

pub trait From<T> {
    fn from(T) -> Self;
}"#,
            r#"enum A { One(u32), Two(String), }

<|>impl From<u32> for A {
    fn from(v: u32) -> Self {
        A::One(v)
    }
}

impl From<String> for A {
    fn from(v: String) -> Self {
        A::Two(v)
    }
}

pub trait From<T> {
    fn from(T) -> Self;
}"#,
        );
    }
}
