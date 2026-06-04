<?php

require __DIR__ . '/../vendor/autoload.php';

$app = require_once __DIR__ . '/../bootstrap/app.php';
$kernel = $app->make(Illuminate\Contracts\Console\Kernel::class);
$kernel->bootstrap();

use App\Models\Plan;
use App\Models\Knowledge;
use App\Models\ServerGroup;
use App\Models\User;
use App\Utils\Helper;
use Illuminate\Support\Facades\File;

if (!User::where('email', 'admin@local')->exists()) {
    $user = new User();
    $user->email = 'admin@local';
    $user->password = password_hash('12345678', PASSWORD_DEFAULT);
    $user->uuid = Helper::guid(true);
    $user->token = Helper::guid();
    $user->is_admin = 1;
    $user->save();
    echo "[seed] admin user created: admin@local / 12345678\n";
}

if (!ServerGroup::query()->exists()) {
    $group = new ServerGroup();
    $group->name = 'Default Group';
    $group->save();
    echo "[seed] server group created: id={$group->id}\n";
    $groupId = $group->id;
} else {
    $groupId = ServerGroup::query()->orderBy('id')->value('id');
}

if (!Plan::query()->exists()) {
    $plan = new Plan();
    $plan->group_id = $groupId;
    $plan->transfer_enable = 100;
    $plan->name = 'Test Plan';
    $plan->show = 1;
    $plan->sort = 1;
    $plan->renew = 1;
    $plan->content = 'Local docker test plan';
    $plan->month_price = 100;
    $plan->quarter_price = 280;
    $plan->half_year_price = 540;
    $plan->year_price = 1000;
    $plan->onetime_price = 9900;
    $plan->save();
    echo "[seed] plan created: id={$plan->id}\n";
}

if (!Knowledge::query()->exists()) {
    $knowledge = new Knowledge();
    $knowledge->language = 'zh-CN';
    $knowledge->category = '使用文档';
    $knowledge->title = '本地开发环境快速开始';
    $knowledge->body = <<<'MARKDOWN'
# 本地开发环境快速开始

这是 Docker 本地开发环境的默认文档，用来避免知识库为空时页面看起来像白屏。

- 用户端：http://localhost:5173
- 管理端：http://localhost:5174
- 测试账号：admin@local
- 测试密码：12345678

如果需要验证订阅购买流程，可以从“购买订阅”进入默认的 Test Plan。
MARKDOWN;
    $knowledge->sort = 1;
    $knowledge->show = 1;
    $knowledge->save();
    echo "[seed] knowledge article created: id={$knowledge->id}\n";
}

$configPath = base_path('config/v2board.php');
if (!File::exists($configPath)) {
    $config = [
        'app_name'             => 'V2Board',
        'app_description'      => 'V2Board is best',
        'app_url'              => 'http://localhost:8000',
        'subscribe_url'        => '',
        'email_verify'         => 1,
        'stop_register'        => 0,
        'invite_force'         => 0,
        'invite_commission'    => 10,
        'recaptcha_enable'     => 0,
        'telegram_bot_enable'  => 0,
        'currency'             => 'CNY',
        'currency_symbol'      => '¥',
        'show_info_to_server_enable' => 0,
        'frontend_admin_path'  => 'admin',
        'secure_path'          => 'admin',
    ];
    File::put($configPath, "<?php\n return " . var_export($config, true) . " ;");
    echo "[seed] config/v2board.php written (email_verify=1, telegram/recaptcha disabled, admin path=/admin)\n";
}

echo "[seed] done\n";
