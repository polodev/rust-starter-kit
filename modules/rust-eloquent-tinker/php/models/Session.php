<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\BelongsTo;

/** A web login. Only the SHA-256 of the cookie token is stored. */
class Session extends BaseModel
{
    protected $table = 'sessions';

    protected $hidden = ['token_hash'];

    protected $casts = ['expires_at' => 'datetime', 'last_used_at' => 'datetime'];

    public function user(): BelongsTo
    {
        return $this->belongsTo(User::class, 'user_id');
    }
}
