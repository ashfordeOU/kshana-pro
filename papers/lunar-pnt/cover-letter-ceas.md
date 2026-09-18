16 July 2026

To: The Editor-in-Chief, *CEAS Space Journal*

**Re: Submission of an original research article —**
*"Earth-baseline VLBI restores the observability of a lunar surface station in joint orbit-and-clock determination"*

Dear Editor,

I am submitting the above manuscript for consideration as an original research article in the *CEAS Space Journal*. The work sits squarely within the journal's scope of guidance, navigation and control, mission and space-systems design, satellite constellations, and deep-space and planetary exploration: it addresses when, and by what measurement, the absolute position of a lunar surface station can be determined within a small cislunar PNT network.

The contribution is threefold. First, the manuscript proves that the null space of purely internal ranging — station-to-satellite and inter-satellite, together with clock synchronisation — in a joint orbit-and-clock fit is exactly the six-degree-of-freedom rigid-body datum defect: the network's internal geometry is well determined, but its absolute position and orientation are not, and the clocks are fully observable so the residual defect is purely positional. Second, it shows that an Earth-frame tie closes this defect, and that the two available ties are not interchangeable — an indirect Earth-to-satellite tie reaches the station only when the constellation geometry is rich enough, whereas a single direct Earth-baseline VLBI delay fixes the station regardless. This yields a conditional design law rather than a single number: VLBI *restores* absolute observability when the constellation cannot supply it and merely *sharpens* the bound when it can, with the absolute datum closing at three non-collinear Earth stations. Third, a Fisher-information / Cramér–Rao analysis — implemented in an engine independently validated against NumPy's linear-algebra routines and published closed-form bounds — quantifies the achievable accuracy along the design curve and confirms that the Gauss–Newton estimator attains it (efficiency 1.02), so the results are information-theoretic limits, not solver artefacts.

I want to be explicit about scope, in keeping with the journal's standards. This is a modelled simulation study. The estimation and information-theoretic machinery is externally validated, but the lunar network it is applied to is a representative closed-loop simulation, not a fit to real tracking, VLBI, or flight data; the observability result therefore stays Modelled, and the manuscript says so plainly. The work is not affiliated with, nor endorsed by, any space agency, and it makes no technology-readiness or heritage claim. Every figure and table is deterministic and rebuilds from a committed scenario, seed list, and pinned engine version; the engine is open source (AGPL-3.0-only) and archived at Zenodo (DOI 10.5281/zenodo.20528627).

A preprint of this work is available on arXiv (arXiv:2607.02566). The manuscript was transferred to *CEAS Space Journal* through the Springer Nature Transfer Desk following a scope-based editorial decision at *Space Science Reviews*; it has not been peer-reviewed elsewhere, is not under consideration at any other journal, and is original work. I declare one relevant interest, stated in full in the manuscript: I am the founder of Ashforde OÜ, which develops the open-source engine used to produce these results and offers it under a commercial licence alongside the AGPL. There are no other competing interests.

I would be grateful for the journal's consideration and am happy to suggest reviewers on request.

Yours sincerely,

Chakshu Baweja
Ashforde OÜ, Tallinn, Estonia
contact@ashforde.org
ORCID: 0009-0008-2098-0751
