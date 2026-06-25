<!DOCTYPE html>
<html>

<head>
    @php
        $assetVersion = function ($path) use ($version) {
            $assetPath = public_path($path);
            return file_exists($assetPath) ? filemtime($assetPath) : $version;
        };
    @endphp
    <link rel="stylesheet" href="/assets/admin/umi.css?v={{$assetVersion('assets/admin/umi.css')}}">
    @if (file_exists(public_path("/assets/admin/custom.css")))
        <link rel="stylesheet" href="/assets/admin/custom.css?v={{$assetVersion('assets/admin/custom.css')}}">
    @endif
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,minimum-scale=1,user-scalable=no">
    <title>{{$title}}</title>
    <!-- <link rel="stylesheet" href="https://fonts.googleapis.com/css?family=Nunito+Sans:300,400,400i,600,700"> -->
    <script>window.routerBase = "/";</script>
    <script>
        window.settings = {
            title: '{{$title}}',
            theme: {
                sidebar: '{{$theme_sidebar}}',
                header: '{{$theme_header}}',
                color: '{{$theme_color}}',
            },
            version: '{{$version}}',
            background_url: '{{$background_url}}',
            logo: '{{$logo}}',
            secure_path: '{{$secure_path}}'
        }
    </script>
</head>

<body>
<div id="root"></div>
<script src="/assets/admin/umi.js?v={{$assetVersion('assets/admin/umi.js')}}"></script>
@if (file_exists(public_path("/assets/admin/custom.js")))
    <script src="/assets/admin/custom.js?v={{$assetVersion('assets/admin/custom.js')}}"></script>
@endif
</body>

</html>
