# EU Declaration of Conformity (DRAFT / UNSIGNED)

**WARNING:** This is an **engineering draft template only**. It is **not signed**, is **not** a binding EU Declaration of Conformity, and must not be published as a completed DoC. Signature and market use require the manufacturer signatory and counsel (see [../legal-track.md](../legal-track.md)).

---

## Draft text (for counsel review)

**EU Declaration of Conformity**  
**(DRAFT - NOT SIGNED)**

1. **Product**  
   Trade name: CPN (Control Panel Network)  
   Product type: Software product with digital elements (hosting control panel and installer)  
   Version / batch covered: _[fill release tag range, e.g. vX.Y.Z and subsequent builds in the technical file]_  

2. **Name and address of the manufacturer**  
   _[Legal entity name]_  
   _[Postal address in the Union / EEA or authorised representative as advised by counsel]_  
   Contact: _[public compliance / PSIRT contact; do not put private phone numbers in git if sensitive]_  

3. **This declaration is issued under the sole responsibility of the manufacturer.**

4. **Object of the declaration**  
   CPN Control Panel Network as described in the technical documentation referenced below.

5. **The object of the declaration described above is in conformity with the relevant Union harmonisation legislation:**  
   Regulation (EU) 2024/2847 (Cyber Resilience Act), to the extent applicable to the versions placed on the market after the applicable application dates, subject to completed conformity assessment.

6. **References to the relevant harmonised standards used or to other technical specifications**  
   _[List when CEN/CENELEC CRA harmonised standards are cited; see harmonised-standards.md]_  
   Other specifications: product SECURITY.md, USER-SECURITY.md, release SBOM and signing practices documented in the technical file.

7. **Where applicable: notified body**  
   Name: _[NANDO body]_  
   Identification number: _[ ]_  
   Certificate / assessment reference: _[ ]_  
   Or: Module / internal production control path as confirmed by counsel if Class I standards path allows.

8. **Additional information**  
   Technical documentation: `to-do/cra/technical-file/` and linked evidence.  
   Support period and user instructions: `docs/SUPPORT.md`, `docs/USER-SECURITY.md`.  
   Vulnerability reporting: `SECURITY.md`.

Signed for and on behalf of: _[manufacturer]_  

Place: _[ ]_  
Date: _[dd/mm/yyyy]_  
Name: _[ ]_  
Function: _[ ]_  
**Signature: NOT AFFIXED IN THIS DRAFT**

---

## Engineering checklist before counsel signs

- [ ] Classification memo filed
- [ ] Annex VII outline filled with evidence links
- [ ] Risk assessment and Annex I matrix have no open Critical engineering gaps for the claimed version range
- [ ] Conformity assessment path completed (standards and/or notified body)
- [ ] User instructions and support period published at acquisition points
- [ ] Signed copy stored in the manufacturer vault (not in this git repository)
