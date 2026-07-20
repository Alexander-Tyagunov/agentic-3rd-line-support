//! Deliberately buggy inventory — the planted defect the coding agent fixes.
//! See CLAUDE.md ("The known planted bug").

/// Error returned when a reservation cannot be satisfied.
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
mod tests {
    use super::*;

    #[test]
    fn test_reserve_success() {
        let mut inv = Inventory::new(10);
        assert_eq!(inv.reserve(4), Ok(6));
        assert_eq!(inv.available, 6);
    }

    #[test]
    fn test_reserve_insufficient_stock_fails() {
        let mut inv = Inventory::new(3);
        assert_eq!(
            inv.reserve(5),
            Err(ReserveError::Insufficient {
                requested: 5,
                available: 3
            })
        );
        // Stock must remain unchanged
        assert_eq!(inv.available, 3);
    }
}
