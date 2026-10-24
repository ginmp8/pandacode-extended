"""Additional frozen regressions, without modifying the original 13 cases."""
import importlib.util, pathlib, sys, unittest
path=pathlib.Path(__file__).with_name('browser-checks.py')
spec=importlib.util.spec_from_file_location('review_browser_base',path)
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
# Only the supplemental fixture adds a delayed catalog response.
base.HTML=base.HTML.replace('loadProviders:async()=>structuredClone(window.fx.providers),', 'loadProviders:async()=>{if(window.fx.delayCatalog)await new Promise(r=>window.fx.releaseCatalog=r);return structuredClone(window.fx.providers);},')
class Regressions(base.BrowserChecks):
    def test_14_changed_derived_preview_requires_new_confirmation(self):
        self.tab('Handoff');self.page.get_by_label('Goal',exact=True).fill('Review parser')
        self.page.get_by_label('I reviewed this preview for sensitive information.',exact=True).check()
        self.page.evaluate("fx.task.title='Changed title not previously reviewed'")
        self.page.get_by_role('button',name='Copy handoff',exact=True).click()
        self.assertEqual(self.page.evaluate('fx.clipboard'),[])
        self.assertFalse(self.page.get_by_label('I reviewed this preview for sensitive information.',exact=True).is_checked())
    def test_15_preparation_freezes_request_before_async_catalog(self):
        self.valid_prompt();self.page.evaluate('fx.delayCatalog=true')
        self.page.get_by_role('button',name='Insert prompt',exact=True).click()
        self.page.wait_for_function('typeof fx.releaseCatalog === "function"')
        self.page.get_by_label('Review question',exact=True).fill('A different unsubmitted question')
        self.page.evaluate('fx.delayCatalog=false;fx.releaseCatalog()')
        self.page.wait_for_function('fx.insertions.length===1')
        result=self.page.evaluate('fx.insertions[0].text')
        self.assertIn('Review parser correctness without editing files.',result)
        self.assertNotIn('A different unsubmitted question',result)
if __name__=='__main__':
    unittest.main(argv=[sys.argv[0],*sys.argv[3:]],verbosity=2)
