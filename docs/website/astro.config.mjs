import {defineConfig} from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
    site: 'https://httpmock.rs',
    // The method pages that the docs.rs API reference replaced.
    redirects: {
        '/matching_requests/path/': '/matching_requests/',
        '/matching_requests/method/': '/matching_requests/',
        '/matching_requests/query/': '/matching_requests/',
        '/matching_requests/headers/': '/matching_requests/',
        '/matching_requests/body/': '/matching_requests/',
        '/matching_requests/cookies/': '/matching_requests/',
        '/matching_requests/host/': '/matching_requests/',
        '/matching_requests/port/': '/matching_requests/',
        '/matching_requests/scheme/': '/matching_requests/',
        '/mocking_responses/all/': '/mocking_responses/',
        '/mocking_responses/delay/': '/mocking_responses/',
    },
    integrations: [
        starlight({
            title: 'httpmock Tutorial',
            logo: {
                light: './src/assets/logo-light.svg',
                dark: './src/assets/logo-dark.svg',
                replacesTitle: true,
            },
            social: [
              { icon: 'github', label: 'GitHub', href: 'https://github.com/httpmock/httpmock' },
              { icon: 'discord', label: 'Discord', href: 'https://discord.gg/BMyywgTJWK' }
            ],
            components: {
                SocialIcons: './src/components/SocialIcons.astro',
            },
            sidebar: [
                {
                    label: 'Getting Started',
                    items: [
                        // Each item here is one entry in the navigation menu.
                        {label: 'Quick Introduction', link: '/getting_started/quick_introduction/'},
                        {label: 'Fundamentals', link: '/getting_started/fundamentals/'},
                        {label: 'Resources', link: '/getting_started/resources/'},
                    ],
                },
                {
                    label: 'Mocking',
                    items: [
                        {label: 'Matching Requests', link: '/matching_requests/'},
                        {label: 'Custom Matchers', link: '/matching_requests/custom/'},
                        {label: 'Mocking Responses', link: '/mocking_responses/'},
                        {label: 'Custom Responses', link: '/mocking_responses/custom/'},
                    ],
                },
                {
                    label: 'Record and Playback',
                    items: [
                        {label: 'Recording', link: '/record-and-playback/recording/'},
                        {label: 'Playback', link: '/record-and-playback/playback/'},
                    ],
                },
                {
                    label: 'Server',
                    items: [
                        {label: 'Standalone Server', link: '/server/standalone/'},
                        {label: 'HTTPS', link: '/server/https/'},
                        {label: 'Debugging', link: '/server/debugging/'},
                    ],
                },
                {
                    label: 'Miscellaneous',
                    items: [
                        {label: 'FAQ', link: '/miscellaneous/faq/'},
                        {label: 'Upgrading', link: '/miscellaneous/upgrading/'},
                        {label: 'License', link: 'https://github.com/httpmock/httpmock/blob/master/LICENSE'},
                    ],
                },
            ],
            customCss: ['./src/assets/landing.css'],
        }),
    ],
});
