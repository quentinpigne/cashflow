-- Adresse propre à une transaction (R-TRX-59, ADR-0029) : au plus une par transaction, mêmes contraintes que
-- l'adresse d'un tiers (R-TIE-23, R-TIE-25). Une table à part évite de recréer ledger_transaction.
CREATE TABLE transaction_address (
    transaction_id TEXT PRIMARY KEY REFERENCES ledger_transaction (id) ON DELETE CASCADE,
    address_line   TEXT CHECK (length(address_line) BETWEEN 1 AND 200),
    postal_code    TEXT CHECK (length(postal_code) BETWEEN 1 AND 10),
    city           TEXT NOT NULL CHECK (length(city) BETWEEN 1 AND 80),
    country_code   TEXT NOT NULL CHECK (length(country_code) = 2 AND country_code = upper(country_code)),
    -- Millionièmes de degré, jamais de flottant (R-TIE-25).
    latitude_e6    INTEGER CHECK (latitude_e6 BETWEEN -90000000 AND 90000000),
    longitude_e6   INTEGER CHECK (longitude_e6 BETWEEN -180000000 AND 180000000),
    CHECK ((latitude_e6 IS NULL) = (longitude_e6 IS NULL))
) STRICT;
