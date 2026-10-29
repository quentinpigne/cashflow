//! Pièces à fournir d'une demande de crédit (R-CRE-17, R-CRE-18) : catalogue, documents trouvés dans la
//! bibliothèque, état de chaque pièce.

use chrono::Months;
use serde::{Deserialize, Serialize};

use super::ProjectKind;
use crate::documents::{DocumentKind, LinkKind, LinkTarget};
use crate::shared::Date;
use crate::shared::ids::DocumentId;

/// Une pièce du catalogue (R-CRE-17).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Piece {
    Identity,
    ProofOfAddress,
    Payslips,
    EmploymentContract,
    TaxNotices,
    BankStatements,
    SavingsStatement,
    LoanDocuments,
    SaleAgreement,
    Quotes,
    CoBorrowerIdentity,
    CoBorrowerPayslips,
    CoBorrowerContract,
}

impl Piece {
    pub const ALL: [Self; 13] = [
        Self::Identity,
        Self::ProofOfAddress,
        Self::Payslips,
        Self::EmploymentContract,
        Self::TaxNotices,
        Self::BankStatements,
        Self::SavingsStatement,
        Self::LoanDocuments,
        Self::SaleAgreement,
        Self::Quotes,
        Self::CoBorrowerIdentity,
        Self::CoBorrowerPayslips,
        Self::CoBorrowerContract,
    ];

    /// Code stocké (`loan_application_piece.piece`).
    pub fn code(self) -> &'static str {
        match self {
            Self::Identity => "IDENTITY",
            Self::ProofOfAddress => "PROOF_OF_ADDRESS",
            Self::Payslips => "PAYSLIPS",
            Self::EmploymentContract => "EMPLOYMENT_CONTRACT",
            Self::TaxNotices => "TAX_NOTICES",
            Self::BankStatements => "BANK_STATEMENTS",
            Self::SavingsStatement => "SAVINGS_STATEMENT",
            Self::LoanDocuments => "LOAN_DOCUMENTS",
            Self::SaleAgreement => "SALE_AGREEMENT",
            Self::Quotes => "QUOTES",
            Self::CoBorrowerIdentity => "CO_BORROWER_IDENTITY",
            Self::CoBorrowerPayslips => "CO_BORROWER_PAYSLIPS",
            Self::CoBorrowerContract => "CO_BORROWER_CONTRACT",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.code() == code)
    }

    /// Libellé, qui nomme aussi le dossier de la pièce dans l'export (R-CRE-19).
    pub fn label(self) -> &'static str {
        match self {
            Self::Identity => "Pièce d'identité",
            Self::ProofOfAddress => "Justificatif de domicile de moins de 3 mois",
            Self::Payslips => "3 dernières fiches de paie",
            Self::EmploymentContract => "Contrat de travail",
            Self::TaxNotices => "2 derniers avis d'imposition",
            Self::BankStatements => "3 derniers relevés de chaque compte courant",
            Self::SavingsStatement => "Relevé de l'épargne qui finance l'apport",
            Self::LoanDocuments => "Offre ou tableau d'amortissement de chaque prêt en cours",
            Self::SaleAgreement => "Compromis de vente",
            Self::Quotes => "Devis des travaux",
            Self::CoBorrowerIdentity => "Pièce d'identité du co-emprunteur",
            Self::CoBorrowerPayslips => "3 dernières fiches de paie du co-emprunteur",
            Self::CoBorrowerContract => "Contrat de travail du co-emprunteur",
        }
    }

    /// Les pièces du co-emprunteur ne sont pas cherchées : elles s'attribuent à la main.
    pub fn is_co_borrower(self) -> bool {
        matches!(self, Self::CoBorrowerIdentity | Self::CoBorrowerPayslips | Self::CoBorrowerContract)
    }
}

/// Ce qui décide des pièces demandées et des documents trouvés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceContext {
    pub project: ProjectKind,
    pub co_borrower: bool,
    pub has_down_payment: bool,
    pub has_works: bool,
    /// Bien de la demande.
    pub asset: Option<uuid::Uuid>,
    /// Emplois en cours.
    pub employments: Vec<uuid::Uuid>,
    /// Comptes courants ouverts.
    pub checking_accounts: Vec<uuid::Uuid>,
    /// Comptes d'épargne ouverts.
    pub savings_accounts: Vec<uuid::Uuid>,
    /// Prêts en cours.
    pub loans: Vec<uuid::Uuid>,
    pub today: Date,
}

/// Un document de la bibliothèque, ce qu'il faut pour le trouver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDocument {
    pub id: DocumentId,
    pub kind: Option<DocumentKind>,
    /// Date de référence (R-DOC-8).
    pub reference_date: Date,
    pub links: Vec<LinkTarget>,
}

impl LibraryDocument {
    fn linked_to(&self, kind: LinkKind, ids: &[uuid::Uuid]) -> bool {
        self.links.iter().any(|l| l.kind == kind && ids.contains(&l.id))
    }
}

/// Pièces demandées pour cette demande, avec le nombre de documents attendus (R-CRE-17).
pub fn required(ctx: &PieceContext) -> Vec<(Piece, usize)> {
    let mut pieces = vec![(Piece::Identity, 1), (Piece::ProofOfAddress, 1), (Piece::Payslips, 3)];
    pieces.push((Piece::EmploymentContract, 1));
    pieces.push((Piece::TaxNotices, 2));
    if !ctx.checking_accounts.is_empty() {
        pieces.push((Piece::BankStatements, 3 * ctx.checking_accounts.len()));
    }
    if ctx.has_down_payment {
        pieces.push((Piece::SavingsStatement, 1));
    }
    if !ctx.loans.is_empty() {
        pieces.push((Piece::LoanDocuments, ctx.loans.len()));
    }
    if ctx.project.is_home_purchase() {
        pieces.push((Piece::SaleAgreement, 1));
    }
    if ctx.has_works {
        pieces.push((Piece::Quotes, 1));
    }
    if ctx.co_borrower {
        pieces.extend([(Piece::CoBorrowerIdentity, 1), (Piece::CoBorrowerPayslips, 3), (Piece::CoBorrowerContract, 1)]);
    }
    pieces
}

/// Les `count` documents les plus récents (date de référence) qui répondent au filtre.
fn latest(library: &[LibraryDocument], count: usize, keep: impl Fn(&LibraryDocument) -> bool) -> Vec<&LibraryDocument> {
    let mut found: Vec<&LibraryDocument> = library.iter().filter(|d| keep(d)).collect();
    found.sort_by(|a, b| b.reference_date.cmp(&a.reference_date).then(b.id.cmp(&a.id)));
    found.truncate(count);
    found
}

/// Documents de la bibliothèque trouvés pour une pièce (R-CRE-17, dernière colonne). Aucun pour les pièces du
/// co-emprunteur.
pub fn found(piece: Piece, ctx: &PieceContext, library: &[LibraryDocument]) -> Vec<DocumentId> {
    let of_kind = |kind: DocumentKind| move |d: &LibraryDocument| d.kind == Some(kind);
    let documents: Vec<&LibraryDocument> = match piece {
        Piece::Identity => latest(library, 1, of_kind(DocumentKind::Identity)),
        Piece::ProofOfAddress => {
            let since = ctx.today.checked_sub_months(Months::new(3)).unwrap_or(ctx.today);
            latest(library, 1, |d| d.kind == Some(DocumentKind::ProofOfAddress) && d.reference_date >= since)
        }
        Piece::Payslips => latest(library, 3, |d| {
            d.kind == Some(DocumentKind::Payslip) && d.linked_to(LinkKind::Employment, &ctx.employments)
        }),
        Piece::EmploymentContract => latest(library, 1, |d| {
            d.kind == Some(DocumentKind::Contract) && d.linked_to(LinkKind::Employment, &ctx.employments)
        }),
        Piece::TaxNotices => latest(library, 2, of_kind(DocumentKind::Tax)),
        Piece::BankStatements => ctx
            .checking_accounts
            .iter()
            .flat_map(|account| {
                latest(library, 3, move |d| {
                    d.kind == Some(DocumentKind::BankStatement) && d.linked_to(LinkKind::Account, &[*account])
                })
            })
            .collect(),
        Piece::SavingsStatement => latest(library, 1, |d| {
            d.kind == Some(DocumentKind::BankStatement) && d.linked_to(LinkKind::Account, &ctx.savings_accounts)
        }),
        Piece::LoanDocuments => ctx
            .loans
            .iter()
            .flat_map(|loan| {
                latest(library, 1, move |d| {
                    matches!(d.kind, Some(DocumentKind::LoanOffer | DocumentKind::AmortizationSchedule))
                        && d.linked_to(LinkKind::Loan, &[*loan])
                })
            })
            .collect(),
        Piece::SaleAgreement | Piece::Quotes => {
            let kind = if piece == Piece::SaleAgreement { DocumentKind::SaleAgreement } else { DocumentKind::Quote };
            let assets: Vec<uuid::Uuid> = ctx.asset.into_iter().collect();
            let count = if piece == Piece::SaleAgreement { 1 } else { usize::MAX };
            latest(library, count, |d| d.kind == Some(kind) && d.linked_to(LinkKind::Asset, &assets))
        }
        Piece::CoBorrowerIdentity | Piece::CoBorrowerPayslips | Piece::CoBorrowerContract => Vec::new(),
    };
    documents.into_iter().map(|d| d.id).collect()
}

/// État d'une pièce (R-CRE-18).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PieceStatus {
    Provided,
    Incomplete,
    Missing,
    NotRequired,
}

/// État d'une pièce selon ses documents (attribués, sinon trouvés) et le nombre attendu.
pub fn status(expected: usize, documents: usize, not_required: bool) -> PieceStatus {
    if not_required {
        PieceStatus::NotRequired
    } else if documents == 0 {
        PieceStatus::Missing
    } else if documents < expected {
        PieceStatus::Incomplete
    } else {
        PieceStatus::Provided
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn uuid(n: u128) -> uuid::Uuid {
        uuid::Uuid::from_u128(n)
    }

    fn doc(n: u128, kind: DocumentKind, date: &str, links: &[(LinkKind, u128)]) -> LibraryDocument {
        LibraryDocument {
            id: DocumentId::from_uuid(uuid(1000 + n)),
            kind: Some(kind),
            reference_date: d(date),
            links: links.iter().map(|(kind, id)| LinkTarget { kind: *kind, id: uuid(*id) }).collect(),
        }
    }

    fn ctx() -> PieceContext {
        PieceContext {
            project: ProjectKind::ExistingHome,
            co_borrower: false,
            has_down_payment: true,
            has_works: false,
            asset: Some(uuid(5)),
            employments: vec![uuid(1)],
            checking_accounts: vec![uuid(2), uuid(3)],
            savings_accounts: vec![uuid(4)],
            loans: vec![uuid(6)],
            today: d("2026-10-29"),
        }
    }

    #[test]
    fn r_cre_17_required_pieces_depend_on_the_application() {
        let pieces = required(&ctx());
        assert!(pieces.contains(&(Piece::BankStatements, 6)), "3 par compte courant");
        assert!(pieces.contains(&(Piece::SaleAgreement, 1)));
        assert!(pieces.contains(&(Piece::LoanDocuments, 1)));
        assert!(!pieces.iter().any(|(p, _)| *p == Piece::Quotes));
        assert!(!pieces.iter().any(|(p, _)| p.is_co_borrower()));
        let other = PieceContext {
            project: ProjectKind::Works,
            co_borrower: true,
            has_down_payment: false,
            has_works: true,
            loans: vec![],
            ..ctx()
        };
        let pieces = required(&other);
        assert!(pieces.contains(&(Piece::Quotes, 1)));
        assert!(pieces.contains(&(Piece::CoBorrowerPayslips, 3)));
        assert!(!pieces.iter().any(|(p, _)| matches!(p, Piece::SaleAgreement | Piece::SavingsStatement)));
        for piece in Piece::ALL {
            assert_eq!(Piece::parse(piece.code()), Some(piece));
        }
    }

    #[test]
    fn r_cre_17_documents_are_found_in_the_library() {
        use DocumentKind::*;
        use LinkKind::*;
        let library = vec![
            doc(1, Payslip, "2026-08-01", &[(Employment, 1)]),
            doc(2, Payslip, "2026-09-01", &[(Employment, 1)]),
            doc(3, Payslip, "2026-10-01", &[(Employment, 1)]),
            doc(4, Payslip, "2026-07-01", &[(Employment, 1)]),
            doc(5, Payslip, "2026-10-01", &[(Employment, 9)]),
            doc(6, ProofOfAddress, "2026-06-15", &[]),
            doc(7, ProofOfAddress, "2026-08-15", &[]),
            doc(8, BankStatement, "2026-09-01", &[(Account, 2)]),
            doc(9, BankStatement, "2026-09-01", &[(Account, 4)]),
            doc(10, SaleAgreement, "2026-10-10", &[(Asset, 5)]),
            doc(11, Tax, "2025-09-01", &[]),
        ];
        let ids = |piece| found(piece, &ctx(), &library);
        let id = |n: u128| DocumentId::from_uuid(uuid(1000 + n));
        assert_eq!(ids(Piece::Payslips), [id(3), id(2), id(1)], "les 3 plus récentes de l'emploi en cours");
        assert_eq!(ids(Piece::ProofOfAddress), [id(7)], "moins de 3 mois");
        assert_eq!(ids(Piece::BankStatements), [id(8)]);
        assert_eq!(ids(Piece::SavingsStatement), [id(9)]);
        assert_eq!(ids(Piece::SaleAgreement), [id(10)]);
        assert_eq!(ids(Piece::TaxNotices), [id(11)]);
        assert!(ids(Piece::Identity).is_empty());
        assert!(ids(Piece::CoBorrowerPayslips).is_empty());
    }

    #[test]
    fn r_cre_18_piece_status() {
        assert_eq!(status(3, 3, false), PieceStatus::Provided);
        assert_eq!(status(2, 1, false), PieceStatus::Incomplete);
        assert_eq!(status(1, 0, false), PieceStatus::Missing);
        assert_eq!(status(1, 0, true), PieceStatus::NotRequired);
    }
}
