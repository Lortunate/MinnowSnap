# Article replay fixture

`article.png` is a 1400 x 3200 crop of a locally generated HTML test document,
rendered in Chromium at device scale 2. The text was authored for this test;
it contains numbered sections, repeated paragraphs, blank lines and callouts.
It is not a capture of user content or an external website.

Keep this raster stable: tests replay integer crops and compare every committed
RGBA pixel with the corresponding document crop. They do not depend on local
fonts, a browser installation, or a platform screenshot permission. The fixture
covers text and whitespace that noise-only tests miss; it does not establish
native screen-capture or compositor exclusion correctness.
