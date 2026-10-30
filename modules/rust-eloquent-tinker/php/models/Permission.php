<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Relations\BelongsToMany;

class Permission extends BaseModel
{
    protected $table = 'permissions';

    protected $casts = ['is_active' => 'boolean'];

    public function roles(): BelongsToMany
    {
        return $this->belongsToMany(Role::class, 'permission_roles', 'permission_id', 'role_id')->withTimestamps();
    }
}
