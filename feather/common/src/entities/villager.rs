// This file is @generated. Please do not edit.
use quill::entities::Villager;
use vane::EntityBuilder;
pub fn build_default(builder: &mut EntityBuilder) {
    super::build_default(builder);
    builder.add(Villager);
}
