/// Пространственный блок между тиками τ.
/// Базовые типы (блоки, координаты).

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
#[repr(C, align(16))]
pub struct Block {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) z: u32,
    _pad: u32,
}

impl core::fmt::Debug for Block {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Block")
            .field("x", &self.x)
            .field("y", &self.y)
            .field("z", &self.z)
            .finish()
    }
}

impl Block {
    /// Создать блок (x,y,z).
    #[inline]
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z, _pad: 0 }
    }

    /// Нулевой блок.
    #[inline]
    pub const fn zero() -> Self {
        Self {
            x: 0,
            y: 0,
            z: 0,
            _pad: 0,
        }
    }

    /// Координата x.
    #[inline]
    pub const fn x(&self) -> u32 {
        self.x
    }

    /// Координата y.
    #[inline]
    pub const fn y(&self) -> u32 {
        self.y
    }

    /// Координата z.
    #[inline]
    pub const fn z(&self) -> u32 {
        self.z
    }

    /// L1-норма (x+y+z).
    #[inline]
    pub const fn l1(&self) -> u32 {
        let xy = match self.x.checked_add(self.y) {
            Some(value) => value,
            None => panic!("Block::l1 overflow"),
        };
        match xy.checked_add(self.z) {
            Some(value) => value,
            None => panic!("Block::l1 overflow"),
        }
    }

    /// Покомпонентное сложение блоков.
    #[inline]
    pub const fn add(&self, other: Block) -> Block {
        Block {
            x: match self.x.checked_add(other.x) {
                Some(value) => value,
                None => panic!("Block::add overflow"),
            },
            y: match self.y.checked_add(other.y) {
                Some(value) => value,
                None => panic!("Block::add overflow"),
            },
            z: match self.z.checked_add(other.z) {
                Some(value) => value,
                None => panic!("Block::add overflow"),
            },
            _pad: 0,
        }
    }

    /// Покомпонентная разность блоков.
    #[inline]
    pub const fn sub(&self, other: Block) -> Block {
        Block {
            x: match self.x.checked_sub(other.x) {
                Some(value) => value,
                None => panic!("Block::sub underflow"),
            },
            y: match self.y.checked_sub(other.y) {
                Some(value) => value,
                None => panic!("Block::sub underflow"),
            },
            z: match self.z.checked_sub(other.z) {
                Some(value) => value,
                None => panic!("Block::sub underflow"),
            },
            _pad: 0,
        }
    }
}
