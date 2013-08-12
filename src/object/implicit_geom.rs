use nalgebra::traits::scalar_op::{ScalarAdd, ScalarSub, ScalarDiv};
use nalgebra::traits::translation::Translation;
use ncollide::bounding_volume::aabb::{HasAABB, AABB};
use ncollide::bounding_volume::bounding_volume::LooseBoundingVolume;
use ncollide::geom::implicit::Implicit;
use ncollide::geom::ball;
use ncollide::geom::plane;
use ncollide::geom::compound::CompoundAABB;
use object::volumetric::Volumetric;

/// Enumeration grouping all common shapes. Used to simplify collision detection
/// dispatch.
// FIXME: #[deriving(Clone)]
pub enum DefaultGeom<N, V, M, II> { // FIXME: rename that
    Plane(plane::Plane<V>),
    Ball(ball::Ball<N>),
    Compound(@CompoundAABB<N, V, M, DefaultGeom<N, V, M, II>>),
    Implicit(~DynamicImplicit<N, V, M, II>)
}

impl<N, V, M, II> DefaultGeom<N, V, M, II> {
    #[inline]
    pub fn new_ball(b: ball::Ball<N>) -> DefaultGeom<N, V, M, II> {
        Ball(b)
    }

    #[inline]
    pub fn new_plane(p: plane::Plane<V>) -> DefaultGeom<N, V, M, II> {
        Plane(p)
    }

    #[inline]
    pub fn new_implicit<I: 'static + Send + DynamicImplicit<N, V, M, II>>(i: ~I)
        -> DefaultGeom<N, V, M, II> {
        Implicit(i as ~DynamicImplicit<N, V, M, II>)
    }

    #[inline]
    pub fn new_compound(c: @CompoundAABB<N, V, M, DefaultGeom<N, V, M, II>>)
        -> DefaultGeom<N, V, M, II> {
        Compound(c)
    }
}

impl<N, V, M, II> DefaultGeom<N, V, M, II> {
    /**
     * Convenience method to extract a ball from the enumation. Fails if the
     * pattern `Ball` is not matched.
     */
    #[inline]
    pub fn ball<'r>(&'r self) -> &'r ball::Ball<N> {
        match *self {
            Ball(ref b) => b,
            _ => fail!("Unexpected geometry: this is not a ball.")
        }
    }

    /**
     * Convenience method to extract a plane from the enumation. Fails if the
     * pattern `Plane` is not matched.
     */
    #[inline]
    pub fn plane<'r>(&'r self) -> &'r plane::Plane<V> {
        match *self {
            Plane(ref p) => p,
            _ => fail!("Unexpected geometry: this is not a plane.")
        }
    }

    #[inline]
    pub fn implicit<'r>(&'r self) -> &'r ~DynamicImplicit<N, V, M, II> {
        match *self {
            Implicit(ref i) => i,
            _ => fail!("Unexpected geometry: this is not an implicit.")
        }
    }

    #[inline]
    pub fn compound(&self) -> @CompoundAABB<N, V, M, DefaultGeom<N, V, M, II>> {
        match *self {
            Compound(c) => c,
            _ => fail!("Unexpected geometry: this is not a compound.")
        }
    }
}

impl<N: NumCast,
     V: Bounded + Neg<V> + ScalarAdd<N> + ScalarSub<N> + ScalarDiv<N> + Orderable + Ord + Clone,
     M: Translation<V> + Mul<M, M>,
     II>
HasAABB<N, V, M> for DefaultGeom<N, V, M, II> {
    fn aabb(&self, m: &M) -> AABB<N, V> {
        match *self {
            Plane(ref p)    => p.aabb(m).loosened(NumCast::from(0.08f64)),
            Ball(ref b)     => b.aabb(m).loosened(NumCast::from(0.08f64)),
            Compound(ref c) => c.aabb(m).loosened(NumCast::from(0.08f64)),
            Implicit(ref i) => i.aabb(m).loosened(NumCast::from(0.08f64))
        }
    }
}

pub trait DynamicImplicit<N, V, M, II> : Implicit<V, M> + Volumetric<N, V, II> + HasAABB<N, V, M> {
    // FIXME: those methods are workarounds: why dont trait objects of this
    // traits dont inherit from all the parent traits?
    fn _support_point(&self, m: &M, dir: &V) -> V;
    fn _volume(&self)                        -> N;
    fn _mass_properties(&self, &N)           -> (N, V, II);
    fn _aabb(&self, &M)                      -> AABB<N, V>;
}

impl<T: Implicit<V, M> + Volumetric<N, V, II> + HasAABB<N, V, M>,
     V,
     N,
     M,
     II>
DynamicImplicit<N, V, M, II> for T {
    #[inline]
    fn _support_point(&self, m: &M, dir: &V) -> V {
        self.support_point(m, dir)
    }

    #[inline]
    fn _volume(&self) -> N {
        self.volume()
    }

    #[inline]
    fn _mass_properties(&self, mass: &N) -> (N, V, II) {
        self.mass_properties(mass)
    }

    #[inline]
    fn _aabb(&self, m: &M) -> AABB<N, V> {
        self.aabb(m)
    }
}

// FIXME: all the following are workarounds to make
// ~ImplicitVolumetricTransformationBoundingVolume implement all the traits it
// inherits from. This is a compiler issue.
impl<N, V, M, II> Implicit<V, M> for ~DynamicImplicit<N, V, M, II> {
    #[inline]
    fn support_point(&self, m: &M, dir: &V) -> V {
        self._support_point(m, dir)
    }
}

impl<N, V, M, II> Volumetric<N, V, II> for ~DynamicImplicit<N, V, M, II> {
    #[inline]
    fn volume(&self) -> N {
        self._volume()
    }

    #[inline]
    fn mass_properties(&self, density: &N) -> (N, V, II) {
        self._mass_properties(density)
    }
}

impl<N, V, M, II> HasAABB<N, V, M> for ~DynamicImplicit<N, V, M, II> {
    fn aabb(&self, m: &M) -> AABB<N, V> {
        self._aabb(m)
    }
}
