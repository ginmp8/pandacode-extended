"""Isolated fixture checks for explicit manual multi-harness UI and native readiness gate."""
import importlib.util, pathlib, sys
path=pathlib.Path(__file__).with_name('browser-checks.py')
spec=importlib.util.spec_from_file_location('review_browser_base_0354',path)
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
base.HTML=base.HTML.replace(
    'loadProviders:async()=>structuredClone(window.fx.providers),',
    "reviewReadiness:async()=>({status:'containment_unproven',ready:false,provenProviders:[],note:'No OS proof'}),loadProviders:async()=>structuredClone(window.fx.providers),"
)

class Browser0354(base.BrowserChecks):
    def add_claude(self):
        self.page.evaluate("fx.providers.push({id:'claude',name:'Claude',installed:true,path:'/fixture/claude',version:'fixture',models:[{id:'claude-model',name:'Claude model',description:''}]})")
        self.page.get_by_role('button',name='Refresh snapshot',exact=True).click()
        self.page.get_by_label('Coordinator provider',exact=True).select_option('claude')
        self.page.get_by_label('Coordinator model',exact=True).select_option('claude-model')

    def test_16_native_gate_is_visible_and_does_not_launch_anything(self):
        state=self.page.locator('.review-isolation')
        self.assertIn('containment_unproven',state.inner_text())
        self.assertIn('disabled',state.inner_text())
        self.assertEqual(self.page.evaluate('fx.commands'),[])
        state.get_by_role('button',name='Check isolation readiness').click()
        self.assertEqual(self.page.evaluate('fx.insertions'),[])

    def test_17_mixed_provider_profile_is_saved_through_original_api(self):
        self.add_claude()
        self.page.get_by_label('Reviewer 1 provider',exact=True).select_option('codex')
        self.page.get_by_label('Reviewer 1 model',exact=True).select_option('fixture-model')
        self.valid_prompt()
        self.page.get_by_role('button',name='Save team profile',exact=True).click()
        self.page.wait_for_function('fx.profiles.length===1')
        profile=self.page.evaluate('fx.profiles[0].setup')
        self.assertEqual(profile['architect']['provider'],'claude')
        self.assertEqual(profile['architect']['model'],'claude-model')
        self.assertEqual(profile['config']['workers'][0]['provider'],'codex')
        self.assertTrue(self.page.get_by_role('button',name='Insert prompt',exact=True).is_disabled())

    def test_18_committee_risk_role_is_explicit_and_bounded(self):
        self.add_claude()
        self.page.get_by_label('Preset',exact=True).select_option('committee')
        self.page.get_by_label('Reviewer 1 provider',exact=True).select_option('codex')
        self.page.get_by_label('Reviewer 1 model',exact=True).select_option('fixture-model')
        self.assertTrue(self.page.get_by_label('Reviewer 2 provider',exact=True).is_visible())
        self.valid_prompt()
        self.page.get_by_role('button',name='Save team profile',exact=True).click()
        self.page.wait_for_function('fx.profiles.length===1')
        setup=self.page.evaluate('fx.profiles[0].setup')
        self.assertEqual([setup['config']['maxParallel'],setup['config']['maxAssignments'],setup['config']['maxRounds']],[2,2,2])
        self.assertEqual([w['provider'] for w in setup['config']['workers']],['codex','claude'])

if __name__=='__main__':
    import unittest
    unittest.main(argv=[sys.argv[0],*sys.argv[3:]],verbosity=2)
