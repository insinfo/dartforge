mod before { pub struct Set(pub Vec<u64>); impl Set {
    pub fn elements(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| {
            (0..64)
                .filter(move |b| w >> b & 1 == 1)
                .map(move |b| k * 64 + b)
        })
    }
}}
mod after { pub struct Set(pub Vec<u64>); impl Set {
    pub fn elements(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(k, &w)| {
            let mut remaining = w;
            std::iter::from_fn(move || {
                if remaining == 0 {
                    return None;
                }
                let bit = remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                // Ascending indices preserve deterministic slot coloring.
                Some(k * 64 + bit)
            })
        })
    }
}}


mod candidate {
 pub struct Set(pub Vec<u64>);
 impl Set {
  pub fn elements(&self)->impl Iterator<Item=usize>+'_ {
   self.0.iter().enumerate().flat_map(|(k,&w)| {
    let prefix=w&w.wrapping_add(1)==0;
    let end=if prefix {w.trailing_ones() as usize}else{0};
    let mut next=0;let mut remaining=w;
    std::iter::from_fn(move|| {
     let bit=if prefix {
      if next==end{return None;}let bit=next;next+=1;bit
     }else{
      if remaining==0{return None;}let bit=remaining.trailing_zeros() as usize;remaining&=remaining-1;bit
     };
     Some(k*64+bit)
    })
   })
  }
 }
}

use std::hint::black_box;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let variant=&args[1];let case=&args[2];
    let words:Vec<u64>=(0..1024).map(|i|match case.as_str(){
        "empty"=>0,"sparse"=>if i%4==0 {1u64<<((i*17)%64)}else{0},
        "dense"=>u64::MAX,"near_dense"=>u64::MAX^(1u64<<((i*17)%64)),_=>panic!("unknown case")
    }).collect();
    let old=before::Set(words.clone());let new=after::Set(words.clone());let candidate=candidate::Set(words);
    assert_eq!(old.elements().collect::<Vec<_>>(),new.elements().collect::<Vec<_>>());
    assert_eq!(old.elements().collect::<Vec<_>>(),candidate.elements().collect::<Vec<_>>());
    let start=std::time::Instant::now();let mut checksum=0u64;
    for _ in 0..512 {
        let sum=if variant=="before"{black_box(&old).elements().fold(0u64,|s,v|s.wrapping_add(v as u64))}
        else if variant=="candidate"{black_box(&candidate).elements().fold(0u64,|s,v|s.wrapping_add(v as u64))}
        else{black_box(&new).elements().fold(0u64,|s,v|s.wrapping_add(v as u64))};
        checksum=checksum.wrapping_add(black_box(sum));
    }
    println!("{} {} {} {}",variant,case,start.elapsed().as_secs_f64(),checksum);
}
