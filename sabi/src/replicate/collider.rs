use bevy::math::Vec3;
use bevy::prelude::*;
use bevy_rapier3d::prelude::Collider;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Serialize, Deserialize, Replicate)]
#[replicate(crate = "crate")]
pub enum ColliderLoader {
    Ball {
        radius: f32,
    },
    HalfSpace {
        outward_normal: Vec3,
    },
    Cylinder {
        half_height: f32,
        radius: f32,
    },
    RoundCylinder {
        half_height: f32,
        radius: f32,
        border_radius: f32,
    },
    Cone {
        half_height: f32,
        radius: f32,
    },
    RoundCone {
        half_height: f32,
        radius: f32,
        border_radius: f32,
    },
    Cuboid {
        hx: f32,
        hy: f32,
        hz: f32,
    },
    RoundCuboid {
        hx: f32,
        hy: f32,
        hz: f32,
        border_radius: f32,
    },
    Capsule {
        a: Vec3,
        b: Vec3,
        radius: f32,
    },
    CapsuleX {
        half_height: f32,
        radius: f32,
    },
    CapsuleY {
        half_height: f32,
        radius: f32,
    },
    CapsuleZ {
        half_height: f32,
        radius: f32,
    },
    Segment {
        a: Vec3,
        b: Vec3,
    },
    Triangle {
        a: Vec3,
        b: Vec3,
        c: Vec3,
    },
    RoundTriangle {
        a: Vec3,
        b: Vec3,
        c: Vec3,
        border_radius: f32,
    },
    // These would bloat the size of this by a lot, so leaving it out for now.
    //Polyline
    //Trimesh
}

impl ColliderLoader {
    pub fn as_collider(&self) -> Collider {
        match *self {
            Self::Ball { radius } => Collider::ball(radius),
            Self::HalfSpace { outward_normal } => Collider::halfspace(outward_normal).unwrap(),
            Self::Cylinder {
                half_height,
                radius,
            } => Collider::cylinder(half_height, radius),
            Self::RoundCylinder {
                half_height,
                radius,
                border_radius,
            } => Collider::round_cylinder(half_height, radius, border_radius),
            Self::Cone {
                half_height,
                radius,
            } => Collider::cone(half_height, radius),
            Self::RoundCone {
                half_height,
                radius,
                border_radius,
            } => Collider::round_cone(half_height, radius, border_radius),
            Self::Cuboid { hx, hy, hz } => Collider::cuboid(hx, hy, hz),
            Self::RoundCuboid {
                hx,
                hy,
                hz,
                border_radius,
            } => Collider::round_cuboid(hx, hy, hz, border_radius),
            Self::Capsule { a, b, radius } => Collider::capsule(a, b, radius),
            Self::CapsuleX {
                half_height,
                radius,
            } => Collider::capsule_x(half_height, radius),
            Self::CapsuleY {
                half_height,
                radius,
            } => Collider::capsule_y(half_height, radius),
            Self::CapsuleZ {
                half_height,
                radius,
            } => Collider::capsule_z(half_height, radius),
            Self::Segment { a, b } => Collider::segment(a, b),
            Self::Triangle { a, b, c } => Collider::triangle(a, b, c),
            Self::RoundTriangle {
                a,
                b,
                c,
                border_radius,
            } => Collider::round_triangle(a, b, c, border_radius),
        }
    }
}

pub fn load_collider(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    query: Query<(Entity, &ColliderLoader), Changed<ColliderLoader>>,
) {
    for (entity, loader) in query.iter() {
        println!("load collider");
        //commands.entity(entity).remove::<MeshLoader>();

        commands.entity(entity).insert(loader.as_collider());
    }
}
