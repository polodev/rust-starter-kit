<?php

/*
 * Eloquent, the `DB` facade and PsySH against the Rust app's database. Started by `cargo run -- task tinker:eloquent`,
 * which passes the connection in DB_* environment variables (taken from the app's config).
 *
 *   php tinker.php                      interactive shell
 *   TINKER_EXEC='User::count()' php tinker.php     run one expression and print the result
 */

require __DIR__ . '/vendor/autoload.php';

use Illuminate\Container\Container;
use Illuminate\Database\Capsule\Manager as Capsule;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Events\Dispatcher;
use Psy\Configuration;
use Psy\Shell;

$capsule = new Capsule;
$capsule->addConnection([
    'driver' => 'mysql',
    'host' => getenv('DB_HOST') ?: '127.0.0.1',
    'port' => getenv('DB_PORT') ?: '3306',
    'database' => getenv('DB_DATABASE') ?: '',
    'username' => getenv('DB_USERNAME') ?: 'root',
    'password' => getenv('DB_PASSWORD') ?: '',
    'charset' => 'utf8mb4',
    'collation' => 'utf8mb4_unicode_ci',
    'prefix' => '',
]);
$capsule->setEventDispatcher(new Dispatcher(new Container));
$capsule->setAsGlobal();
$capsule->bootEloquent();

// Small Laravel helpers that live in the framework package, which this console does not load.
if (!function_exists('now')) {
    function now(): Illuminate\Support\Carbon
    {
        return Illuminate\Support\Carbon::now();
    }
}
if (!function_exists('today')) {
    function today(): Illuminate\Support\Carbon
    {
        return Illuminate\Support\Carbon::today();
    }
}

// The `DB` facade of Laravel: DB::table('users')->where(...)->get(), DB::select(...), DB::statement(...),
// DB::transaction(fn () => ...), DB::beginTransaction() / DB::rollBack() and so on.
class_alias(Capsule::class, 'DB');

/**
 * The Rust app hashes passwords with Argon2id, not bcrypt, so `bcrypt()` / `Hash::make()` hashes
 * would never log in. Use this (the User model does it for you when you assign `password`).
 */
function rust_password(string $plain): string
{
    return password_hash($plain, PASSWORD_ARGON2ID);
}

// Give every model a short global name, so `User::first()` works like it does in artisan tinker.
foreach (glob(__DIR__ . '/models/*.php') as $file) {
    $name = basename($file, '.php');
    if ($name !== 'BaseModel' && !class_exists($name, false)) {
        class_alias("App\\Models\\{$name}", $name);
    }
}

$config = new Configuration([
    'updateCheck' => 'never',
    'usePcntl' => false,
    'startupMessage' => '<info>Eloquent tinker on ' . getenv('DB_DATABASE') . ". Facade: DB::table('users')->get(). Models: "
        . implode(', ', array_map(fn ($f) => basename($f, '.php'), array_filter(glob(__DIR__ . '/models/*.php'), fn ($f) => basename($f) !== 'BaseModel.php')))
        . '. Passwords: rust_password(\'secret\').</info>',
]);

// Show models as their attributes and collections as their items, instead of Eloquent's internals.
$config->getPresenter()->addCasters([
    Model::class => fn (Model $model) => $model->attributesToArray(),
    Illuminate\Support\Collection::class => fn (Illuminate\Support\Collection $items) => $items->all(),
]);

$shell = new Shell($config);

$code = getenv('TINKER_EXEC');
if ($code !== false && $code !== '') {
    $shell->setOutput(new Symfony\Component\Console\Output\ConsoleOutput());
    $result = $shell->execute($code, true);
    $shell->writeReturnValue($result);
    exit(0);
}

$shell->run();
