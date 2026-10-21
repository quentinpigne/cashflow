-- Option d'arrondi des intérêts à chaque quinzaine, si une banque procède ainsi (R-EPA-6).
ALTER TABLE fixed_savings_terms
    ADD COLUMN round_each_fortnight INTEGER NOT NULL DEFAULT 0 CHECK (round_each_fortnight IN (0, 1));
