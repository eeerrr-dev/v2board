<?php

require __DIR__ . '/../vendor/autoload.php';

$app = require_once __DIR__ . '/../bootstrap/app.php';
$kernel = $app->make(Illuminate\Contracts\Console\Kernel::class);
$kernel->bootstrap();

use App\Models\Plan;
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
    echo "[seed] admin user created: admin@local / 123456\n";
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
