<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\BelongsToMany;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Support\Str;
use Ramsey\Uuid\Uuid;

class User extends BaseModel
{
    protected $table = 'users';

    protected $hidden = ['pid', 'password', 'api_key', 'reset_token', 'email_verification_token', 'magic_link_token'];

    protected $casts = [
        'email_verified_at' => 'datetime',
        'reset_sent_at' => 'datetime',
        'email_verification_sent_at' => 'datetime',
        'magic_link_expiration' => 'datetime',
    ];

    protected static function booted(): void
    {
        // The Rust app needs these two columns on every user.
        static::creating(function (User $user) {
            // `pid` is a 16-byte binary uuid in the Rust app's table.
            $user->pid ??= Uuid::uuid4()->getBytes();
            $user->api_key ??= 'lo-' . Str::uuid();
        });
    }

    /** The public id as text (the column holds 16 raw bytes). */
    public function getPublicIdAttribute(): ?string
    {
        return $this->attributes['pid'] ?? null ? Uuid::fromBytes($this->attributes['pid'])->toString() : null;
    }

    /** Argon2id, because that is what the Rust app verifies. Already-hashed values pass through. */
    public function setPasswordAttribute(string $value): void
    {
        $this->attributes['password'] = str_starts_with($value, '$argon2') ? $value : rust_password($value);
    }

    public function roles(): BelongsToMany
    {
        return $this->belongsToMany(Role::class, 'role_users', 'user_id', 'role_id')->withTimestamps();
    }

    public function directPermissions(): BelongsToMany
    {
        return $this->belongsToMany(Permission::class, 'permission_users', 'user_id', 'permission_id')->withTimestamps();
    }

    public function sessions(): HasMany
    {
        return $this->hasMany(Session::class, 'user_id');
    }

    public function tokens(): HasMany
    {
        return $this->hasMany(PersonalAccessToken::class, 'user_id');
    }

    public function socialAccounts(): HasMany
    {
        return $this->hasMany(SocialAccount::class, 'user_id');
    }

    /** The user's avatar rows in the media library (collection "logo"). */
    public function logo()
    {
        return Media::where('model_type', 'user')->where('model_id', (string) $this->id)->where('collection_name', 'logo');
    }
}
