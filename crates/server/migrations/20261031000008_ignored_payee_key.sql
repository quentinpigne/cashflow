-- Clés de tiers ignorées (TIE-05, R-TIE-35) : leurs propositions de tiers ne sont plus faites. Les propositions
-- elles-mêmes se recalculent (R-TIE-31) ; la clé identifie la ligne, qui se supprime pour la rétablir.
CREATE TABLE ignored_payee_key (
    key        TEXT PRIMARY KEY CHECK (length(key) BETWEEN 1 AND 200),
    ignored_at TEXT NOT NULL CHECK (ignored_at LIKE '%Z')
) STRICT;
