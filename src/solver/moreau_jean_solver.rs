use slab::Slab;

use na::{DVector, RealField};
use ncollide::query::ContactId;

use crate::counters::Counters;
use crate::detection::ColliderContactManifold;
use crate::joint::JointConstraint;
use crate::object::{BodyHandle, BodySlab};
use crate::material::MaterialsCoefficientsTable;
use crate::solver::{ConstraintSet, ContactModel, IntegrationParameters, NonlinearSORProx, SORProx};
use crate::world::ColliderWorld;

/// Moreau-Jean time-stepping scheme.
pub struct MoreauJeanSolver<N: RealField> {
    jacobians: Vec<N>,
    // FIXME: use a Vec or a DVector?
    mj_lambda_vel: DVector<N>,
    ext_vels: DVector<N>,
    contact_model: Box<ContactModel<N>>,
    contact_constraints: ConstraintSet<N, ContactId>,
    joint_constraints: ConstraintSet<N, usize>,
    internal_constraints: Vec<BodyHandle>,
}

impl<N: RealField> MoreauJeanSolver<N> {
    /// Create a new time-stepping scheme with the given contact model.
    pub fn new(contact_model: Box<ContactModel<N>>) -> Self {
        MoreauJeanSolver {
            jacobians: Vec::new(),
            mj_lambda_vel: DVector::zeros(0),
            ext_vels: DVector::zeros(0),
            contact_model,
            contact_constraints: ConstraintSet::new(),
            joint_constraints: ConstraintSet::new(),
            internal_constraints: Vec::new(),
        }
    }

    /// Sets the contact model.
    pub fn set_contact_model(&mut self, model: Box<ContactModel<N>>) {
        self.contact_model = model
    }

    /// Perform one step of the time-stepping scheme.
    pub fn step(
        &mut self,
        counters: &mut Counters,
        bodies: &mut BodySlab<N>,
        joints: &mut Slab<Box<JointConstraint<N>>>,
        manifolds: &[ColliderContactManifold<N>],
        island: &[BodyHandle],
        params: &IntegrationParameters<N>,
        coefficients: &MaterialsCoefficientsTable<N>,
        cworld: &ColliderWorld<N>,
    ) {
        counters.assembly_started();
        self.assemble_system(counters, params, coefficients, bodies, joints, manifolds, island);
        counters.assembly_completed();

        counters.set_nconstraints(self.contact_constraints.velocity.len() + self.joint_constraints.velocity.len());

        println!("Solving velocity constraints.");
        counters.velocity_resolution_started();
        self.solve_velocity_constraints(params, bodies);
        println!("Caching impulses.");
        self.cache_impulses(bodies, joints);
        counters.velocity_resolution_completed();

        counters.velocity_update_started();
        println!("Updating velocities.");
        self.update_velocities_and_integrate(params, bodies, island);
        counters.velocity_update_completed();

        counters.position_resolution_started();
        println!("Solving position constraints.");
        self.solve_position_constraints(params, cworld, bodies, joints);
        counters.position_resolution_completed();
    }

    // FIXME: this comment is bad.
    /// Perform one sub-step of the time-stepping scheme as part of a CCD integration.
    pub fn step_ccd(
        &mut self,
        counters: &mut Counters,
        bodies: &mut BodySlab<N>,
        joints: &mut Slab<Box<JointConstraint<N>>>,
        manifolds: &[ColliderContactManifold<N>],
        ccd_pair: [BodyHandle; 2],
        island: &[BodyHandle],
        params: &IntegrationParameters<N>,
        coefficients: &MaterialsCoefficientsTable<N>,
        cworld: &ColliderWorld<N>,
    ) {
        self.assemble_system(counters, params, coefficients, bodies, joints, manifolds, island);
//        for constraint in &mut self.constraints.position.unilateral {
//            if constraint.body1.0 != ccd_pair[0] && constraint.body1.0 != ccd_pair[1] {
//                constraint.ndofs1 = 0;
//            }
//
//            if constraint.body2.0 != ccd_pair[0] && constraint.body2.0 != ccd_pair[1] {
//                constraint.ndofs2 = 0;
//            }
//        }

        self.solve_position_constraints(params, cworld, bodies, joints);
        bodies.body_mut(ccd_pair[0]).unwrap().validate_advancement();
        bodies.body_mut(ccd_pair[1]).unwrap().validate_advancement();

//        for handle in island {
//            let body = try_continue!(bodies.body_mut(*handle));
//            body.validate_advancement();
//        }

        self.solve_velocity_constraints(params, bodies);
//        self.cache_impulses(bodies, joints);
        self.update_velocities_and_integrate(params, bodies, island);
    }

    fn assemble_system(
        &mut self,
        counters: &mut Counters,
        params: &IntegrationParameters<N>,
        coefficients: &MaterialsCoefficientsTable<N>,
        bodies: &mut BodySlab<N>,
        joints: &mut Slab<Box<JointConstraint<N>>>,
        manifolds: &[ColliderContactManifold<N>],
        island: &[BodyHandle],
    ) {
        self.internal_constraints.clear();
        let mut system_ndofs = 0;

        for handle in island {
            let body = try_continue!(bodies.body_mut(*handle));
            body.set_companion_id(system_ndofs);
            let ndofs = body.status_dependent_ndofs();
            assert!(
                ndofs != 0,
                "Internal error: an island cannot contain a non-dynamic body."
            );

            system_ndofs += ndofs;

            if ndofs != 0 && body.has_active_internal_constraints() {
                self.internal_constraints.push(*handle)
            }
        }

        println!("System ndofs: {}", system_ndofs);
        println!("Island len: {}", island.len());
        self.resize_buffers(system_ndofs);
        self.contact_constraints.clear();
        self.joint_constraints.clear();

        /*
         * Initialize M^{-1} h * dt
         */
        for handle in island {
            let body = try_continue!(bodies.body(*handle));
            let id = body.companion_id();
            let accs = body.generalized_acceleration();

            self.ext_vels
                .rows_mut(id, accs.len())
                .axpy(params.dt(), &accs, N::zero());
        }

        /*
         *
         * Compute jacobian sizes.
         *
         */
        let mut jacobian_sz = 0;
        let mut ground_jacobian_sz = 0;

        for (_, g) in joints.iter() {
            if g.is_active(bodies) {
                let (b1, b2) = g.anchors();
                let body1 = try_continue!(bodies.body(b1.0));
                let body2 = try_continue!(bodies.body(b2.0));

                let ndofs1 = body1.status_dependent_ndofs();
                let ndofs2 = body2.status_dependent_ndofs();

                let nconstraints = g.num_velocity_constraints();
                let sz = nconstraints * 2 * (ndofs1 + ndofs2);

                if ndofs1 == 0 || ndofs2 == 0 {
                    ground_jacobian_sz += sz;
                } else {
                    jacobian_sz += sz;
                }
            }
        }

        for m in manifolds {
            let ndofs1 = try_continue!(bodies.body(m.body1())).status_dependent_ndofs();
            let ndofs2 = try_continue!(bodies.body(m.body2())).status_dependent_ndofs();
            let sz = self.contact_model.num_velocity_constraints(m) * (ndofs1 + ndofs2) * 2;

            if ndofs1 == 0 || ndofs2 == 0 {
                ground_jacobian_sz += sz;
            } else {
                jacobian_sz += sz;
            }
        }

        self.jacobians
            .resize(jacobian_sz + ground_jacobian_sz, N::zero());

        /*
         *
         * Initialize constraints.
         *
         */
        let mut j_id = 0;
        let mut ground_j_id = jacobian_sz;

        for (_, g) in joints {
            if g.is_active(bodies) {
                g.velocity_constraints(
                    params,
                    bodies,
                    &self.ext_vels,
                    &mut ground_j_id,
                    &mut j_id,
                    &mut self.jacobians,
                    &mut self.joint_constraints,
                );
            }
        }

        counters.custom_started();
        self.contact_model.constraints(
            params,
            coefficients,
            bodies,
            &self.ext_vels,
            manifolds,
            &mut ground_j_id,
            &mut j_id,
            &mut self.jacobians,
            &mut self.contact_constraints,
        );
        counters.custom_completed();


        for handle in &self.internal_constraints {
            if let Some(body) = bodies.body_mut(*handle) {
                let ext_vels = self.ext_vels.rows(body.companion_id(), body.ndofs());
                body.setup_internal_velocity_constraints(&ext_vels, params);
            }
        }

        println!("Constraints setup complete.");
    }

    fn solve_velocity_constraints(&mut self, params: &IntegrationParameters<N>, bodies: &mut BodySlab<N>) {
        SORProx::solve(
            bodies,
            &mut self.contact_constraints.velocity,
            &mut self.joint_constraints.velocity,
            &self.internal_constraints,
            &mut self.mj_lambda_vel,
            &self.jacobians,
            params.max_velocity_iterations,
        );
    }

    fn solve_position_constraints(
        &mut self,
        params: &IntegrationParameters<N>,
        cworld: &ColliderWorld<N>,
        bodies: &mut BodySlab<N>,
        joints: &mut Slab<Box<JointConstraint<N>>>,
    ) {
        // XXX: avoid the systematic clone.
        // This is needed for cases where we perform the position resolution
        // before the velocity resolution.
        let mut jacobians = self.jacobians.clone();
        NonlinearSORProx::solve(
            params,
            cworld,
            bodies,
            &mut self.contact_constraints.position.unilateral,
            joints,
            &self.internal_constraints,
            &mut jacobians,
            params.max_position_iterations,
        );
    }

    fn cache_impulses(
        &mut self,
        bodies: &mut BodySlab<N>,
        joints: &mut Slab<Box<JointConstraint<N>>>,
    ) {
        self.contact_model.cache_impulses(&self.contact_constraints);

        for (_, g) in joints {
            if g.is_active(bodies) {
                g.cache_impulses(&self.joint_constraints);
            }
        }
    }

    fn resize_buffers(&mut self, ndofs: usize) {
        // XXX: use resize functions instead of reallocating.
        self.mj_lambda_vel = DVector::zeros(ndofs);
        self.ext_vels = DVector::zeros(ndofs);
    }

    fn update_velocities_and_integrate(
        &mut self,
        params: &IntegrationParameters<N>,
        bodies: &mut BodySlab<N>,
        island: &[BodyHandle],
    ) {
        for handle in island {
            let body = try_continue!(bodies.body_mut(*handle));
            let id = body.companion_id();
            let ndofs = body.ndofs();

            {
//                println!("ext_vels: {:?}, mj_lambda: {:?}", self.ext_vels, self.mj_lambda_vel);
                let mut mb_vels = body.generalized_velocity_mut();
                mb_vels += self.ext_vels.rows(id, ndofs);
                mb_vels += self.mj_lambda_vel.rows(id, ndofs);
            }

            body.integrate(params);
        }
    }
}
