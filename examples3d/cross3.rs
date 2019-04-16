extern crate nalgebra as na;
extern crate ncollide3d;
extern crate nphysics3d;
extern crate nphysics_testbed3d;

use na::{Isometry3, Point3, Vector3};
use ncollide3d::shape::{Compound, Cuboid, ShapeHandle};
use nphysics3d::object::{ColliderDesc, RigidBodyDesc};
use nphysics3d::world::World;
use nphysics_testbed3d::Testbed;


pub fn init_world(testbed: &mut Testbed) {
    /*
     * World
     */
    let mut world = World::new();
    world.set_gravity(Vector3::new(0.0, -9.81, 0.0));

    /*
     * Ground.
     */
    let ground_thickness = 0.2;
    let ground_shape =
        ShapeHandle::new(Cuboid::new(Vector3::new(3.0, ground_thickness, 3.0)));

    ColliderDesc::new(ground_shape)
        .translation(Vector3::y() * -ground_thickness)
        .build(&mut world);

    /*
     * Cross shaped geometry
     */
    let mut cross_geoms = Vec::new();

    let large_rad = 0.25f32;
    let small_rad = 0.01f32;

    let edge_x = Cuboid::new(Vector3::new(large_rad, small_rad, small_rad));
    let edge_y = Cuboid::new(Vector3::new(small_rad, large_rad, small_rad));
    let edge_z = Cuboid::new(Vector3::new(small_rad, small_rad, large_rad));

    cross_geoms.push((Isometry3::identity(), ShapeHandle::new(edge_x)));
    cross_geoms.push((Isometry3::identity(), ShapeHandle::new(edge_y)));
    cross_geoms.push((Isometry3::identity(), ShapeHandle::new(edge_z)));

    let compound = Compound::new(cross_geoms);
    let cross = ShapeHandle::new(compound);
    let collider_desc = ColliderDesc::new(cross)
        .density(1.0);

    /*
     * Create the crosses
     */
    let num = 5;
    let shift = (large_rad + 0.08) * 2.0;
    let centerx = shift * (num as f32) / 2.0;
    let centery = 3.0 + shift / 2.0;
    let centerz = shift * (num as f32) / 2.0;

    let mut rb_desc = RigidBodyDesc::new()
        .collider(&collider_desc);

    for i in 0usize..num {
        for j in 0usize..num {
            for k in 0usize..num {
                let x = i as f32 * shift - centerx;
                let y = j as f32 * shift + centery;
                let z = k as f32 * shift - centerz;

                // Build the rigid body and its collider.
                rb_desc
                    .set_translation(Vector3::new(x, y, z))
                    .build(&mut world);
            }
        }
    }

    /*
     * Set up the testbed.
     */
    testbed.set_world(world);
    testbed.look_at(Point3::new(-4.0, 1.0, -4.0), Point3::new(0.0, 1.0, 0.0));
}

fn main() {
    let mut testbed = Testbed::new_empty();
    init_world(&mut testbed);
    testbed.run();
}
