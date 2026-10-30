<?php

namespace App\Models;

class Contact extends BaseModel
{
    protected $table = 'contacts';

    protected $casts = ['replied_at' => 'datetime'];

    public function markReplied(): bool
    {
        return $this->update(['status' => 'replied', 'replied_at' => now()]);
    }
}
