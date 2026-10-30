<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\BelongsTo;

class PersonalAccessToken extends BaseModel
{
    protected $table = 'personal_access_tokens';

    protected $hidden = ['token_hash'];

    protected $casts = ['last_used_at' => 'datetime', 'expires_at' => 'datetime'];

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class, 'user_id');
    }
}
