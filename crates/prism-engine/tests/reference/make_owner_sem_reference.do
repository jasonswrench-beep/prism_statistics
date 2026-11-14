* Reference values for the structural equation models in owner_sem_reference.rs, from the product owner's
* command-driven reference package (one of "the other reference packages the product owner has").
* Run in that package's batch mode from this folder; it writes owner_sem_cd.json.
* Standard errors use the expected information, as lavaan's and PrismStatistics' defaults.
* Each value is keyed "<model>.<the package's own parameter name>.<field>"; the test maps the names.
version 19
set more off
tempname f
file open `f' using owner_sem_cd.json, write replace
file write `f' "{" _n
global first 1
* Converged tightly, so the comparison is not limited by where the optimizer stops.
global tight "iterate(2000) tolerance(1e-12) ltolerance(1e-14) nrtolerance(1e-12)"

capture program drop put
program define put
    args fh key value
    if missing(`value') exit
    if $first == 0 file write `fh' "," _n
    global first 0
    * JSON numbers need a digit before the decimal point.
    local s = strtrim(string(`value', "%21.15g"))
    if substr("`s'", 1, 1) == "." local s = "0`s'"
    if substr("`s'", 1, 2) == "-." local s = "-0" + substr("`s'", 2, .)
    file write `fh' `"  "`key'": `s'"'
end

* Every parameter's estimate and standard error (and standardized estimate and standard error), then the fit measures.
capture program drop dump
program define dump
    args fh p
    matrix T = r(table)
    local names : colfullnames T
    local k 0
    foreach nm of local names {
        local ++k
        put `fh' "`p'.`nm'.est" T[1,`k']
        put `fh' "`p'.`nm'.se" T[2,`k']
    }
    put `fh' "`p'.fit.logl" e(ll)
    put `fh' "`p'.fit.npar" e(rank)
    quietly sem, standardized
    matrix S = r(table)
    local k 0
    foreach nm of local names {
        local ++k
        put `fh' "`p'.`nm'.std" S[1,`k']
        put `fh' "`p'.`nm'.std_se" S[2,`k']
    }
    quietly estat gof, stats(all)
    foreach s in chi2_ms df_ms chi2_bs df_bs rmsea lb90_rmsea ub90_rmsea pclose cfi tli srmr aic bic chi2sb_ms chi2sb_bs cfi_sb tli_sb rmsea_sb {
        capture put `fh' "`p'.fit.`s'" r(`s')
    }
end

import delimited holzinger.csv, clear
encode school, generate(sch)

* Three-factor CFA (maximum likelihood).
quietly sem (Visual -> x1 x2 x3) (Textual -> x4 x5 x6) (Speed -> x7 x8 x9), vce(eim) $tight
dump `f' cfa

* The same CFA with the Satorra-Bentler scaled chi-square and robust standard errors.
quietly sem (Visual -> x1 x2 x3) (Textual -> x4 x5 x6) (Speed -> x7 x8 x9), vce(sbentler) $tight
dump `f' mlm

* A path model with observed predictors (their moments taken as given).
quietly sem (x4 <- x1 x2) (x9 <- x4 x1) (x5 <- x2), vce(eim) $tight
dump `f' path

* Two groups: configural (everything free) and metric (loadings equal).
quietly sem (Visual -> x1 x2 x3) (Textual -> x4 x5 x6) (Speed -> x7 x8 x9), group(sch) ginvariant(none) means(Visual@0 Textual@0 Speed@0) vce(eim) $tight
dump `f' mgconfigural
quietly sem (Visual -> x1 x2 x3) (Textual -> x4 x5 x6) (Speed -> x7 x8 x9), group(sch) ginvariant(mcoef) means(Visual@0 Textual@0 Speed@0) vce(eim) $tight
dump `f' mgmetric

* The political democracy model: loadings equal across waves, correlated residuals.
import delimited poldem.csv, clear
quietly sem (Ind60 -> x1 x2 x3) (Dem60 -> y1 y2@a y3@b y4@c) (Dem65 -> y5 y6@a y7@b y8@c) (Dem60 <- Ind60) (Dem65 <- Ind60 Dem60), cov(e.y1*e.y5 e.y2*e.y4 e.y2*e.y6 e.y3*e.y7 e.y4*e.y8 e.y6*e.y8) vce(eim) $tight
dump `f' pd

* A linear latent growth model (observed intercepts 0, latent means free).
import delimited growth.csv, clear
quietly sem (I@1 S@0 _cons@0 -> t1) (I@1 S@1 _cons@0 -> t2) (I@1 S@2 _cons@0 -> t3) (I@1 S@3 _cons@0 -> t4), latent(I S) means(I S) vce(eim) $tight
dump `f' lgm

file write `f' _n "}" _n
file close `f'
exit, clear
