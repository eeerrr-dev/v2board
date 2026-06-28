<!DOCTYPE html>
<html>

<head>
    <script>
        (function () {
            try {
                var mode = document.cookie.split('; ').reduce(function (value, item) {
                    var parts = item.split('=');
                    if (parts[0] !== 'dark_mode' || parts[1] === undefined) return value;
                    try {
                        return decodeURIComponent(parts[1]);
                    } catch (error) {
                        return value;
                    }
                }, '');
                if (mode === '1') {
                    document.documentElement.classList.add('dark');
                    document.documentElement.style.colorScheme = 'dark';
                }
            } catch (error) {}
        })();
    </script>
    @php
        $assetVersion = function ($path) use ($version) {
            $assetPath = public_path($path);
            return file_exists($assetPath) ? filemtime($assetPath) : $version;
        };
    @endphp
    <link rel="stylesheet" href="/theme/{{$theme}}/assets/umi.css?v={{$assetVersion("theme/{$theme}/assets/umi.css")}}">
    @if (file_exists(public_path("/theme/{$theme}/assets/custom.css")))
        <link rel="stylesheet" href="/theme/{{$theme}}/assets/custom.css?v={{$assetVersion("theme/{$theme}/assets/custom.css")}}">
    @endif
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,minimum-scale=1,user-scalable=no">
    @php ($colors = [
        'darkblue' => '#3b5998',
        'black' => '#343a40',
        'default' => '#0665d0',
        'green' => '#319795'
    ])
    <meta name="theme-color" content="{{$colors[$theme_config['theme_color']]}}">

    <title>{{$title}}</title>
    <!-- <link rel="stylesheet" href="https://fonts.googleapis.com/css?family=Nunito+Sans:300,400,400i,600,700"> -->
    <script>window.routerBase = "/";</script>
    <script>
        window.settings = {
            title: '{{$title}}',
            theme: {
                sidebar: '{{$theme_config['theme_sidebar']}}',
                header: '{{$theme_config['theme_header']}}',
                color: '{{$theme_config['theme_color']}}',
            },
            version: '{{$version}}',
            background_url: '{{$theme_config['background_url']}}',
            description: '{{$description}}',
            i18n: [
                'zh-CN',
                'en-US',
                'ja-JP',
                'vi-VN',
                'ko-KR',
                'zh-TW'
            ],
            logo: '{{$logo}}'
        }
    </script>
</head>

<body>
<div id="root"></div>
{!! $theme_config['custom_html'] !!}
<script src="/theme/{{$theme}}/assets/umi.js?v={{$assetVersion("theme/{$theme}/assets/umi.js")}}"></script>
@if (file_exists(public_path("/theme/{$theme}/assets/custom.js")))
    <script src="/theme/{{$theme}}/assets/custom.js?v={{$assetVersion("theme/{$theme}/assets/custom.js")}}"></script>
@endif
</body>

</html>
