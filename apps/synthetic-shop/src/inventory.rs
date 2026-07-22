//! Deliberately buggy inventory — the planted defect the coding agent fixes.
//! See CLAUDE.md ("The known planted bug").

/// Error returned when a reservation cannot be satisfied.
#[allow(dead_code)] // constructed only once the planted bug is fixed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReserveError {
    Insufficient { requested: i64, available: i64 },
}

/// A tiny stock ledger.
#[derive(Debug)]
pub struct Inventory {
    available: i64,
}

impl Inventory {
    pub fn new(stock: i64) -> Self {
        Self { available: stock }
    }

    /// Reserve `qty` units, returning the remaining stock.
    ///
    /// PLANTED BUG: this decrements before verifying availability, so reserving
    /// more than is in stock drives `available` negative (an oversell) instead of
    /// failing. The correct fix: return `Err(ReserveError::Insufficient)` when
    /// `qty > self.available` and leave stock unchanged — and add a regression test.
    pub fn reserve(&mut self, qty: i64) -> Result<i64, ReserveError> {
        if qty > self.available {
            return Err(ReserveError::Insufficient {
                requested: qty,
                available: self.available,
            });
        }
        self.available -= qty;
        Ok(self.available)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_reserve_sufficient_stock() {
        let mut inv = Inventory::new(10);
        let res = inv.reserve(4);
        assert_eq!(res, Ok(6));
        assert_eq!(inv.available, 6);
    }

    #[test]
    fn test_reserve_insufficient_stock() {
        let mut inv = Inventory::new(3);
        let res = inv.reserve(5);
        assert_eq!(
            res,
            Err(ReserveError::Insufficient {
                requested: 5,
                available: 3
            })
        );
        assert_eq!(inv.available, 3);
    }

    #[test]
    fn test_reserve_exact_stock() {
        let mut inv = Inventory::new(5);
        let res = inv.reserve(5);
        assert_eq!(res, Ok(0));
        assert_eq!(inv.available, 0);
    }
}
