-- Date d'achat d'un paiement par carte, lue dans le libellé d'un relevé PDF LCL (R-IMP-26).
ALTER TABLE import_line ADD COLUMN purchase_date TEXT CHECK (purchase_date IS NULL OR purchase_date = date(purchase_date));
