# PandaCode Extended — ownership, licensing and attribution

PandaCode Extended is an independently maintained derivative of the original [PandaCode](https://github.com/aicodeking/pandacode) project. **This is a fork of PandaCode.** It preserves and synchronizes inherited features while developing separate Extended-specific capabilities.

## Inherited source and rights

The repository's root [`LICENSE`](../../LICENSE) is the original PandaCode license. It is MIT-derived **with an additional attribution condition** and must not be presented as standard unmodified MIT. Its original permission, disclaimer, copyright and attribution language remains intact. Repositories, binaries, documentation and user-visible product surfaces must retain the required link and statement identifying the original PandaCode project.

The original authors and other rightsholders retain rights to code and assets inherited from PandaCode. A separate release branch, fork, new features, or a different maintainer does not transfer ownership of those inherited portions. Do not remove upstream license notices, original authors or required third-party acknowledgments, and do not relicense inherited code without appropriate rights.

## Extended-maintained additions

Source files and features developed independently for Extended are maintained in [ginmp8/pandacode-extended](https://github.com/ginmp8/pandacode-extended). Their authorship is distinct from the inherited source where the changes can be identified, but this documentation does not claim exclusive ownership of the entire application or alter any contributor's rights. Such additions remain subject to applicable repository terms unless a specific compatible file-level notice provides otherwise.

Extended-only features and release materials are documented under [`docs/extended/`](README.md), [`releases/extended/`](../../releases/extended/) and the corresponding issues/commits. The maintained upstream parity contract is [here](upstream/parity-contract.md).

## Optional public Agent Skills

[RhapsodIA](https://github.com/ginmp8/rhapsodia) is a separate publicly available skill source, **not** the upstream author of PandaCode. Its root [`LICENSE`](https://github.com/ginmp8/rhapsodia/blob/main/LICENSE) states Apache License 2.0. Every skill and its bundled assets, scripts and dependencies must be inspected independently for package-level `LICENSE`, `NOTICE` and third-party obligations before redistribution.

Importing a skill does not transfer copyright, change the PandaCode license, grant the skill new tool permissions, or authorize execution. RhapsodIA packages are optional, user-initiated imports; the installer contains no RhapsodIA skill packages or recommendation catalog. When importing or redistributing a package, preserve applicable individual notices and distinguish installed content from activated behavior.

The [RhapsodIA import and synchronization specification](rhapsodia-skills.md) is tracked in [issue #125](https://github.com/ginmp8/pandacode-extended/issues/125). No offline RhapsodIA package bundling is planned or shipped.

## Release and display checks

Before releasing, verify that:

- The original license and required upstream attribution remain in source packages, installers and appropriate user-visible product information.
- Download links labeled **Extended** actually point to Extended artifacts; original PandaCode links remain explicitly identified as upstream.
- Each redistributed skill's license and notice files accompany its copied content wherever required.
- Extended provenance does not claim ownership of upstream components, and public documentation does not cite personal-session or private-account sources.

This page documents intended attribution handling and does not replace a formal legal review where distribution rights are disputed.
