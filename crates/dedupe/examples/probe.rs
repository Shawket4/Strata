use dedupe::*;
fn main() {
    let pairs = [("Ahmad Sameer","Ahmed Samir"),("Churn notes","Churn notes v2"),("Churn notes","Churn notes draft"),("Churn notes","Churn notes 2026")];
    for (a,b) in pairs { println!("{a} | {b} => {:?}", score_pair(&Item::entity(DedupeKind::Person,Some("a"),a,&[]), &Item::entity(DedupeKind::Person,Some("b"),b,&[]))); }
}
