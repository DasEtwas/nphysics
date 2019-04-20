extern crate nalgebra as na;
extern crate ncollide2d;
extern crate nphysics2d;
extern crate nphysics_testbed2d;

use na::Vector2;
use ncollide2d::shape::{Cuboid, ShapeHandle};
use nphysics2d::object::{ColliderDesc, RigidBodyDesc};
use nphysics2d::world::World;
use nphysics_testbed2d::Testbed;


pub fn init_world(testbed: &mut Testbed) {
    /*
     * World
     */
    let mut world = World::new();
    world.set_gravity(Vector2::new(0.0, -9.81));

    /*
     * Ground
     */
    let ground_size = 25.0;
    let ground_shape =
        ShapeHandle::new(Cuboid::new(Vector2::new(ground_size, 1.0)));

    ColliderDesc::new(ground_shape)
        .translation(-Vector2::y())
        .build(&mut world);

    /*
     * Create the boxes
     */
    let num = 20;
    let rad = 0.1;

    let cuboid = ShapeHandle::new(Cuboid::new(Vector2::repeat(rad)));
    let collider_desc = ColliderDesc::new(cuboid)
        .density(1.0);

    let mut rb_desc = RigidBodyDesc::new()
        .collider(&collider_desc);

    let shift = 2.0 * (rad + collider_desc.get_margin());
    let centerx = shift * (num as f32) / 2.0;
    let centery = rad + collider_desc.get_margin() * 2.0;

    for i in 0usize..num {
        for j in i..num {
            let fj = j as f32;
            let fi = i as f32;
            let x = (fi * shift / 2.0) + (fj - fi) * shift - centerx;
            let y = fi * shift + centery;

            // Build the rigid body and its collider.
            rb_desc
                .set_translation(Vector2::new(x, y))
                .build(&mut world);
        }
    }

    /*
     * Run the simulation.
     */
    testbed_set_world(world);
}


fn main() {
    let mut testbed = Testbed::new_empty();
    init_world(&mut testbed);
    testbed.run();
}