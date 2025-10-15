import {defineConfig} from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
    site: 'https://httpmock.rs',
    integrations: [
        starlight({
            title: 'httpmock Tutorial',
            logo: {
                light: './src/assets/logo-light.svg',
                dark: './src/assets/logo-dark.svg',
                replacesTitle: true,
            },
            social: {
                github: 'https://github.com/httpmock/httpmock',
                discord: 'https://discord.com/invite/7QzTfBUe',
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
                        // Each item here is one entry in the navigation menu.
                        {
                            label: 'Matching Requests',
                            items: [
                                // Each item here is one entry in the navigation menu.
                                {label: 'Path', link: '/matching_requests/path/'},
                                {label: 'Method', link: '/matching_requests/method/'},
                                {label: 'Query Parameters', link: '/matching_requests/query/'},
                                {label: 'Headers', link: '/matching_requests/headers/'},
                                {label: 'Body', link: '/matching_requests/body/'},
                                {label: 'Cookie', link: '/matching_requests/cookies/'},
                                {label: 'Host', link: '/matching_requests/host/'},
                                {label: 'Port', link: '/matching_requests/port/'},
                                {label: 'Scheme', link: '/matching_requests/scheme/'},
                                {label: 'Custom Matchers', link: '/matching_requests/custom/'},
                            ],

                        },
                        {
                            label: 'Mocking Responses', items: [
                                // Each item here is one entry in the navigation menu.
                                {label: 'Response Values', link: '/mocking_responses/all/'},
                                {label: 'Network Delay', link: '/mocking_responses/delay/'},
                                {label: 'Custom Responses', link: '/mocking_responses/custom/'},
                            ],
                        },
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
                        {label: 'License', link: 'https://github.com/httpmock/httpmock/blob/master/LICENSE'},
                    ],
                },
            ],
            customCss: ['./src/assets/landing.css'],
        }),
    ],
});
